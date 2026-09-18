# Maintainability audit

This is a focused, qualitative audit of production source as of 2026-09-18.
It is not a claim that large files are automatically bad: generated code,
tests, and cohesive implementations (notably the recursive-descent parser)
were deliberately discounted. The ranking instead combines size, number of
responsibilities, mutable state, repeated control flow, and how widely a
change can propagate.

## Priority findings

### 1. Split the live editor into stateful feature controllers

**Why this is the worst offender:** `app/src/live-edit.ts` is 668 lines and
owns several independently changing features: debounced linting, diagnostics
and highlighting, hover documentation, DOM coordinate measurement,
autocomplete state/rendering, edit lifecycle, persistence/compilation, and
event binding. Those features communicate through 11 module-level mutable
bindings (including request counters used to reject stale asynchronous
responses), a module-level cache, and imported mutable DOM/application state
from `code-viewer.ts`.
This makes lifecycle ordering implicit and makes isolated tests depend on
resetting global state correctly.

The clearest seam is to introduce an editor-session object whose lifetime
matches one open file, then extract autocomplete and pointer/measurement code
behind small collaborators. Keep `live-edit.ts` as the event-wiring facade.
Do this incrementally: first move state without changing behavior, then move
the feature functions and their existing tests.

### 2. Decompose the lint build script and centralize code emission

`app/crates/papyrus-lints/build.rs` is 826 lines and implements nine build
pipelines. It repeatedly performs the same read/parse/panic/write sequence,
while the latter half also validates two metadata sources, derives module
names by parsing Rust expressions as strings, and emits entire Rust functions
with `push_str`/`format!`. The generated dispatch functions explicitly suppress
`clippy::too_many_lines`, so changes to rule metadata, validation, imports,
configuration, and execution order all converge on one file.

Split data-table compilation, rule-metadata validation, and dispatch emission
into separate build-script modules. Add shared typed helpers for loading input
and writing generated output. Prefer a small renderer abstraction (or at least
structured line writers) over scattered string concatenation, and unit-test
metadata validation independently from Cargo's build-script execution.

### 3. Replace repeated Tauri command parameter lists and repair workflows

`app/src-tauri/src/repair.rs` has four commands with `clippy::too_many_arguments`
allowances. The mutating commands repeat the same sequence: decode path/source,
construct a function table, apply a selected repair, conditionally write,
prime the cache, and lint again. Each command also independently carries the
same project context (`root`, configuration, lookup roots, compiler path, and
compile-check flag). Adding a project-level option therefore requires changing
multiple frontend/backend command contracts and several nearly identical code
paths.

Define a serializable project/lint context argument shared with the related
lint command, and extract one internal "write, prime, and relint" helper. Keep
the public Tauri command names and response types stable while migrating call
sites, because they are an IPC boundary.

### 4. Separate preset identity, storage, and configuration composition

`app/crates/papyrus-lint-config/src/presets.rs` is 637 lines and combines four
concerns: built-in preset identity, executable-relative path discovery,
user-preset CRUD, and YAML composition/initialization. Its own module-level
documentation has to enumerate all four roles. File lookup and name validation
are consequently coupled to merge and project initialization behavior, and
most filesystem functions need parallel `_under` variants solely to make the
global executable-path dependency testable.

Introduce a small preset-store type rooted at an explicit directory and move
list/read/write/rename/delete operations onto it. Keep built-in preset parsing
and YAML composition separate. Production code can construct the store from
the executable directory, while tests can construct it directly, eliminating
the paired wrapper/internal-function pattern.

## Important, but not first

- `app/crates/papyrus-parser/src/parser.rs` is the largest hand-written
  implementation file at 903 lines, but it is a cohesive recursive-descent
  parser with methods ordered by grammar level. Splitting it now would mostly
  trade local navigation for cross-module navigation. Extract only if a
  concrete grammar area begins changing independently.
- Several lint implementations exceed 400 lines. Most are isolated by rule
  and have sibling test files, so they have a smaller change radius than the
  four findings above. Shared expression-walking infrastructure should be
  introduced only when repeated behavior, rather than similar-looking
  traversal, can be demonstrated.
- Large test files were not ranked as production hotspots. They are worth
  organizing by behavior when navigation becomes painful, but their size does
  not carry the same runtime coupling risk.

## How the review was performed

The initial inventory counted lines in checked-in Rust, TypeScript, and Python
files while excluding dependency/build-output directories, then inspected the
largest production files for responsibility boundaries, branch-heavy code,
global mutable state, repeated workflows, and explicit complexity lint
allowances. `rg` was used to enumerate function/type boundaries and
`too_many_arguments`/`too_many_lines` suppressions. The ranking above comes
from reading the candidates, not from line count alone.

Recommended execution order is the numbered order above. Each item should be
a behavior-preserving refactor with the existing focused test suite kept green;
combining them into one change would create exactly the review risk this audit
is intended to reduce.

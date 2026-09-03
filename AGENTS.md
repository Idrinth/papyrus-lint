# Papyrus Lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app and a CLI. The desktop app is a Tauri (Rust + TypeScript) app: the
frontend lets a user drop a `.achlist` file, or a single `.psc` file
directly, and the Rust backend resolves the achlist's listed files (or
just the single dropped `.psc`), parses whichever `.psc` (Papyrus source)
files result, and lints them. The CLI (`app/crates/papyrus-lint-cli`) does the same thing
non-interactively for an `.achlist`, or lints one `.psc` directly; it also
supports automatic fixes and structured JSON output. The
desktop app's own executable can run either way too: launched with no
arguments it starts the GUI as usual, launched with an `.achlist` path (or
`-h`/`--help`) it delegates straight to the CLI's logic instead
(`app/src-tauri/src/main.rs`) — the standalone `PapyrusLinterCLI` binary stays
available separately for uses (e.g. CI) that shouldn't depend on the
desktop app's binary at all.

## Project structure

```text
.
├── app/                     # The desktop app: Tauri (Rust + TypeScript) shell
│   │                        # and its frontend, with their npm/cargo config
│   ├── src/                  # Frontend (TypeScript, vanilla, no framework)
│   │   ├── main.ts              # Drag-and-drop UI logic, calls into Tauri commands
│   │   ├── highlight.ts         # Standalone Papyrus syntax highlighter for the
│   │   │                        # code viewer dialog
│   │   ├── main.test.ts         # Vitest unit tests for main.ts
│   │   ├── highlight.test.ts    # Vitest unit tests for highlight.ts
│   │   ├── test/fixture.ts      # Shared jsdom DOM fixture for main.test.ts
│   │   └── styles.css
│   ├── index.html            # Frontend entry point (Vite)
│   ├── package.json          # npm scripts/deps for the frontend and Tauri CLI
│   ├── src-tauri/            # Tauri desktop app shell (Rust)
│   │   └── src/
│   │       ├── main.rs           # Binary entry point: no args -> lib::run() (GUI),
│   │       │                     # args -> papyrus_lint_cli::run() (CLI mode)
│   │       ├── lib.rs            # Registers Tauri commands (parse_achlist_file,
│   │       │                     # parse_papyrus_script, lint_papyrus_script,
│   │       │                     # parse_psc_file, load_lint_config, lint_psc_file,
│   │       │                     # repair_psc_file), built on papyrus-lint-core
│   │       ├── compiler.rs        # Runs PapyrusCompiler.exe for the "Compile" button,
│   │       │                     # then strips personal data from the compiled .pex;
│   │       │                     # also compiles into a throwaway temp dir (never
│   │       │                     # touching the project's real output) for the
│   │       │                     # compile_check lint setting, below
│   │       ├── compile_diagnostics.rs # Parses PapyrusCompiler.exe's own reported
│   │       │                     # errors, from compiler.rs's temp-dir compile, into
│   │       │                     # lint Diagnostics for the compile_check setting
│   │       └── pex_header.rs      # Parses a compiled .pex file's header just far
│   │                             # enough to blank its userName/machineName fields
│   └── crates/
│       ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
│       │   └── src/               # for the Papyrus language. No lint rules live
│       │       ├── lexer.rs        # here — see papyrus-lints below.
│       │       ├── token.rs
│       │       ├── ast.rs
│       │       ├── parser.rs
│       │       └── cache.rs        # In-memory memoization of parse()/tokenize()
│       │                           # against the most recently seen source, so
│       │                           # one lint pass over a script only lexes/
│       │                           # parses it once no matter how many lint
│       │                           # rules each ask for their own tokens/AST
│       ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
│       │   ├── build.rs           # (not the AST) so they still run on scripts
│       │   └── src/                # that don't parse cleanly.
│       │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
│       │       ├── config.rs                  # Config type (YAML-deserializable) passed
│       │       │                              # to every check/fix job
│       │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
│       │       ├── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
│       │       │                              # via a build-time-generated array
│       │       └── native_function_usage.rs   # Reads rules/native-methods.yaml via a
│       │                                      # build-time-generated array; disabled by
│       │                                      # default
│       ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
│       │   └── src/               # and the CLI, independent of Tauri:
│       │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
│       │       ├── ast_cache.rs    # Disk-backed cache of parsed .psc ASTs, keyed by
│       │       │                   # content MD5 + mtime + linter version, shared by
│       │       │                   # the desktop app and the CLI (via function_table.rs)
│       │       ├── config.rs       # Locates/loads a project's papyrus-lint.yaml
│       │       ├── script_locator.rs   # Finds .psc files by name under
│       │       │                       # scripts/source or source/scripts
│       │       ├── function_table.rs   # Cross-script function signature lookup,
│       │       │                       # for the argument/return type check lints
│       │       │                       # and script_exists() for the unresolved
│       │       │                       # script reference lint; each signature
│       │       │                       # tracks the State block it came from (if
│       │       │                       # any), preferring the empty state's own
│       │       │                       # declaration over a same-named override
│       │       ├── native_types.rs     # Fallback Extends hierarchy for native
│       │       │                       # engine types (Actor, ObjectReference,
│       │       │                       # Form, ...) with no .psc in the project;
│       │       │                       # reads rules/native-types.yaml via a
│       │       │                       # build-time-generated array (build.rs)
│       │       └── native_globals.rs   # Known native singleton scripts (Game,
│       │                               # Utility, Debug, ...) always called by
│       │                               # literal name, with no .psc in the
│       │                               # project; reads rules/native-globals.yaml
│       │                               # via a build-time-generated array (build.rs)
│       └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-psc>`: lints an
│           └── src/                # achlist's scripts against its project's
│               ├── lib.rs           # papyrus-lint.yaml and prints the results.
│               │                    # run() here is the shared logic; also
│               │                    # linked into src-tauri for its CLI mode.
│               └── main.rs          # Thin binary entry point around lib::run()
├── resources/                # Images used by README.md (logo, screenshots)
├── rules/
│   ├── forbidden-functions.yaml  # Calls discouraged or forbidden by policy
│   ├── slow-functions.yaml       # Slow calls and their faster alternatives
│   ├── native-methods.yaml       # Base-game native functions (see
│   │                             # native_function_usage.rs above); all three
│   │                             # files above are compiled in by
│   │                             # papyrus-lints/build.rs
│   ├── native-types.yaml         # Native engine class hierarchy fallback (see
│   │                              # papyrus-lint-core/src/native_types.rs above);
│   │                              # compiled in by papyrus-lint-core/build.rs
│   └── native-globals.yaml       # Native singleton scripts always called by
│                                  # literal name (see native_globals.rs above);
│                                  # compiled in by papyrus-lint-core/build.rs
├── SublimeLinter-contrib-papyrus-lint/  # Standalone SublimeLinter plugin package,
│   ├── linter.py                          # runs PapyrusLinterCLI against a saved
│   ├── messages.json                      # .psc file and parses its output
│   ├── messages/install.txt               # into SublimeLinter diagnostics; kept
│   ├── README.md                          # here for development but installed/
│   └── LICENSE                            # distributed as its own package.
└── vscode-extension/        # VS Code extension (TypeScript): lints and fixes
    ├── package.json          # .psc files by invoking PapyrusLinterCLI --json
    ├── src/extension.ts      # Commands, process execution, and diagnostics
    └── test/                 # Node-based extension unit tests
```

`papyrus-parser`, `papyrus-lints`, `papyrus-lint-core`, and
`papyrus-lint-cli` are separate crates (not yet Cargo workspace members,
just path dependencies of each other and of `app/src-tauri`) so the lint
engine and project-resolution logic stay reusable independent of the Tauri
app — which is what lets `papyrus-lint-cli` link against them without
pulling in Tauri (and its system GUI dependencies) at all. `app/src-tauri`
depends on `papyrus-lint-cli` too, purely for its `run()` function (its
`main.rs` calls straight into it for CLI mode), not for the `PapyrusLinterCLI`
binary target that crate also defines.

## Development

- Frontend (`app/`): `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (typecheck + build). `npm run test` runs the frontend's
  Vitest unit tests (`src/**/*.test.ts`); `npm run test:coverage` runs the
  same suite instrumented with `@vitest/coverage-v8`, printing a text
  report and writing HTML/lcov reports to `coverage/`. `npm run lint` runs
  ESLint (flat config in `eslint.config.js`) over `src/`, using
  `typescript-eslint`'s recommended rules plus `@vitest/eslint-plugin`'s
  recommended rules on test files.
  - `typescript-eslint` doesn't yet support TypeScript 7 (this repo's
    `typescript` devDependency), so `app/package.json` installs it under an
    npm alias: `typescript` resolves to the `@typescript/typescript6` shim
    (TS 6, satisfying typescript-eslint) and the real TS 7 compiler is
    installed separately as `@typescript/native`, which is what `tsc`
    (used by `npm run build`) actually runs. See
    https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0.
- Full desktop app: `npm run tauri dev` / `npm run tauri build` (from `app/`).
- Rust backend only: `cargo check` / `cargo test` from `app/src-tauri/`.
- Parser crate only: `cargo test` from `app/crates/papyrus-parser/`.
- Lints crate only: `cargo test` from `app/crates/papyrus-lints/`.
- Shared project-resolution crate only: `cargo test` from
  `app/crates/papyrus-lint-core/`.
- CLI: `cargo run --manifest-path app/crates/papyrus-lint-cli/Cargo.toml --
  <path-to-achlist>`, or `cargo build --release --manifest-path
  app/crates/papyrus-lint-cli/Cargo.toml` for a standalone `PapyrusLinterCLI`
  binary (at `app/crates/papyrus-lint-cli/target/release/PapyrusLinterCLI`).
  `cargo test` from `app/crates/papyrus-lint-cli/` runs its tests.
- VS Code extension (`vscode-extension/`): `npm install`, then `npm run
  watch` (or `npm run compile` for a one-off build) and F5 in VS Code to
  launch an Extension Development Host. Not part of the app's npm
  project — it has its own `package.json`/`tsconfig.json`/`eslint.config.js`.
- Rust coverage for any of the five crates above: `cargo llvm-cov
  --manifest-path <crate>/Cargo.toml` (requires the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) subcommand
  and the `llvm-tools-preview` rustup component).

## CI (`.github/workflows/ci.yml`)

- **Rules YAML lint job**: runs `yamllint` against every `rules/*.yaml` file
  so malformed rule data cannot be merged.
- **Nexus page BBCode job**: runs the dependency-free Python BBCode linter's
  unit tests, then checks `docs/nexuspage.bbcode` for unknown, mismatched, and
  unclosed tags.
- **Sublime Text extension job**: runs the plugin's Python unit tests via
  `coverage run -m unittest discover`, scoped to `commands.py`/`linter.py`
  (test files themselves are omitted). The text summary is posted to the
  job's step summary and an lcov report is uploaded as the
  `sublime-extension-coverage` artifact.
- **Frontend job**: in `app/`, `npm ci`, then `npm run lint` (ESLint), `npm
  run test:coverage` (Vitest unit tests, instrumented for coverage), and `npm
  run build` (typecheck & Vite build). The text coverage summary is
  posted to the job's step summary and the full HTML/lcov report is
  uploaded as the `frontend-coverage` artifact.
- **Markdown job**: runs markdownlint-cli2 against every `README.md` in the
  repository, using the root `.markdownlint-cli2.yaml` configuration.
- **VS Code extension job**: installs its dependencies, then runs `npm run
  test:coverage` (the Node test runner's built-in coverage, via
  `--experimental-test-coverage`), ESLint, and TypeScript compilation. The
  text coverage summary is posted to the job's step summary and an lcov
  report is uploaded as the `vscode-extension-coverage` artifact.
- **Rust build job**: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  and `cargo check`, all run against `app/src-tauri/Cargo.toml`.
- **Rust test job**: a matrix over `app/src-tauri`, `app/crates/papyrus-parser`,
  `app/crates/papyrus-lints`, `app/crates/papyrus-lint-core`, and
  `app/crates/papyrus-lint-cli` runs each crate's tests via `cargo llvm-cov`.
  Each matrix leg posts its text coverage summary to the job's step
  summary and uploads its lcov report as a `rust-coverage-<crate>`
  artifact.
- **Coverage summary comment job** (`coverage-comment`, pull requests
  only): downloads every job's lcov artifact and runs
  `.github/scripts/coverage_summary.py` to aggregate line coverage by
  module — crates (the four reusable crates combined), app (`src-tauri`),
  UI (the frontend), and editor plugins (the VS Code extension and the
  Sublime Text plugin combined) — posting the result as a single
  markdown table, updated in place on subsequent pushes, as a PR comment
  (and to the job's step summary). Comment posting is best-effort
  (`continue-on-error`) since forked PRs get a read-only `GITHUB_TOKEN`.

Note: CI runs on pushes to `the-one` (the default branch, not `main`) and
on all pull requests.

## Releases (`.github/workflows/release.yml`)

Pushing a tag matching `v*.*.*` triggers a release job that syncs the
tag's version into `app/src-tauri/tauri.conf.json`, `app/package.json`,
`app/src-tauri/Cargo.toml`, and all four reusable crates' `Cargo.toml` files, then
builds the Tauri desktop app (binary name `PapyrusLinter`) on Linux,
macOS, and Windows (via `tauri-apps/tauri-action`) and the
`PapyrusLinterCLI` CLI binary (via `cargo build --release --manifest-path
app/crates/papyrus-lint-cli/Cargo.toml`) on each platform, attaching each
platform's desktop bundle and CLI binary
(`PapyrusLinterCLI-linux`/`PapyrusLinterCLI-macos`/`PapyrusLinterCLI-windows.exe`)
to a GitHub release for that tag, creating the release if it doesn't
already exist. The `ubuntu-latest` leg also copies the checked-in
`docs/papyrus-lint.default.yaml` (see Configuration above) to
`papyrus-lint.yaml` and attaches it to the release alongside the CLI
binary, rather than generating it by running the freshly built CLI's
`init` subcommand. A separate `editor-plugins` job runs independently,
packages the VS Code extension into a `.vsix` (via `@vscode/vsce`)
and the `SublimeLinter-contrib-papyrus-lint` directory into a `.zip`, and
attaches both to the same release. A final `release-notes` job (after
both `release` and `editor-plugins` succeed) overwrites the release's
title and body — replacing the generic body `tauri-apps/tauri-action`
set on the `release` job — with the tag name as the title; a changelist
of the merged pull requests between the previous and current tag,
resolved per commit via the "list pull requests associated with a
commit" GitHub API and linked with the PR title as text; the current
code coverage (aggregated the same way as CI's coverage-comment job,
via `.github/scripts/coverage_summary.py`, from the lcov artifacts of
the most recent successful `ci.yml` run for the tagged commit); and a
link to the full changelist (`.../compare/<previous-tag>...<tag>`). It
also builds a plain-text version of the same PR changelist (titles only,
no PR numbers or links) and uploads it as the `nexus-changelog` artifact
for the `nexus-upload` job below.

A final `nexus-upload` job (after `release`, `editor-plugins`, and
`release-notes` all succeed) publishes the release to the project's
[Nexus Mods page](https://www.nexusmods.com/skyrimspecialedition/mods/189862)
via the Nexus Mods API, authenticating with the `NEXUSMODS_API_KEY` repo
secret. It downloads the already-built assets straight off the GitHub
release (rather than rebuilding anything) — the Windows installer
(`*setup.exe`), `PapyrusLinterCLI-windows.exe`, the
`docs/papyrus-lint.default.yaml` copy uploaded as `papyrus-lint.yaml`
(zipped locally, since Nexus expects it as an archive), the SublimeLinter
plugin `.zip`, and the VS Code extension
`.vsix` — and posts the `nexus-changelog` artifact's content as a
changelog entry for the tag's version (`POST /mods/{id}/changelogs`).
Each of the five files is then uploaded as a new version of its
corresponding Nexus mod file (`POST /uploads`, a `PUT` of the file to the
returned presigned URL with matching `Content-MD5`, `POST
/uploads/{id}/finalise`, then `POST /mod-files/{id}/versions` once the
upload reports `available`): the two executables as `main` files, the
editor plugins as `optional`, and the zipped config as `miscellaneous`.
The setup.exe upload also sets `primary_mod_manager_download` and
`update_mod_version`, since it's the mod's primary download and drives
the mod-level version shown on the page. The Nexus API has no endpoint
to update a mod's page description, so `docs/nexuspage.bbcode` is not synced
by this job and still needs to be pasted onto the mod page by hand.

## Merging

Before merging a pull request, make sure its branch is up to date with
`the-one` (the default branch). Merge or rebase `the-one` into the branch
first if it has fallen behind, so CI runs against the current base.

## Current state

The parser (`app/crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence. Each parsed `FunctionDecl` records the name of the
`State` block it was declared in (`None` for a function/event declared
directly on the script, i.e. the "empty state"), so downstream tooling can
tell a state override apart from its base declaration; `papyrus-lint-core`'s
`function_table.rs` carries that same `state` field through into its
cross-script function signatures, and includes a function declared only
inside a state (with no matching empty-state declaration) rather than
silently dropping it. Each parsed integer literal (`Literal::Int`) also
records the `IntFormat` (`Decimal` or `Hexadecimal`) it was written with,
so downstream tooling can tell a hex-written literal apart from a decimal
one without re-scanning the original source text; `papyrus-lints`'
`formid-hex-notation` lint reads the same distinction off the lexer's
`TokenKind::IntLiteral` tokens directly, since it works on tokens rather
than the parsed AST. The parsed `Script` also records the line its
`ScriptName` keyword starts on, and a parsed `Stmt::If` records the
line/column its `Else` keyword starts on (`None` when it has no `Else`
clause at all, distinguishing that from an empty `Else` clause, which both
leave `else_body` empty) — so `papyrus-lints`' `property-sorting` and
`empty-body` lints can read that information straight off the AST instead
of re-lexing the source, on a script that parses cleanly at all;
`empty-body`'s `Else` check still falls back to scanning tokens directly
when the script doesn't parse, since that's the only way it can still run
then.

`app/crates/papyrus-lints` currently implements all rules listed in the
[README's Implemented Lints table](README.md#implemented-lints). Rules inspect
raw source or lexer tokens rather than requiring a successfully parsed AST.
Automatic repair is available for trailing whitespace, comma spacing,
semicolons, indentation, whitespace around member-access dots, spacing
around `!` negation, spacing around logical/comparison operators, and
property sorting (disabled by default; see the README). The desktop app,
standalone CLI, and editor extensions all use the same lint and repair
engine.

Project configuration is read from an optional `papyrus-lint.yaml` or
`papyrus-lint.yml` in the project root. Both the desktop app and the CLI are
forgiving of an achlist that doesn't live in the project root itself (e.g.
dropped next to a game's `Data` directory while the project lives in a
subfolder): each first tries to find the root from where the achlist's own
resolved scripts sit under a `Scripts/Source` or `Source/Scripts` pair
(`projectDirForAchlist` in `app/src/main.ts`; `find_candidate_pair_root` in
`papyrus-lint-cli`), falling back to the achlist's own parent directory (the
conventional layout) only if none of them do. The CLI resolves a bare `.psc`
given directly the same way, walking up from the file itself and falling
back to two directories above it if no such pair is found at all; the
desktop app's frontend still uses that simpler fixed "two directories up"
rule for a bare `.psc` dropped directly (`projectDirForPscPath`).

By default, the CLI (and the desktop app) resolves cross-script lookups
among an achlist's own entries by treating every listed entry's parent
directory as a generic additional search root — which matters for achlists
whose entries are grouped into arbitrary source directories rather than
either conventional layout, since a script in one listed directory then
still resolves a script type declared in another listed directory. This
also means an unlisted file that happens to sit in one of those
directories resolves too, and `conflicting_script_versions` scans every
such directory in full, which got expensive on a modlist-sized achlist
(hundreds of listed files across as many directories; see
[#311](https://github.com/Idrinth/papyrus-lint/issues/311)). The CLI avoids
repeating that scan once per listed script by building a
`script_locator::ScriptIndex` (`build_script_index`) — a one-time map from
each directory's file names to their paths — up front, then using it both
for cross-script name resolution and to check every script via
`conflicting_script_versions_in_index` instead of
calling `conflicting_script_versions` (which scans afresh on every call)
per script; the desktop app's single-file `lint_psc_file`/`repair_psc_file`
commands still call `conflicting_script_versions` directly, since they
only ever check one script per invocation. Setting the project's
`strict_achlist_scope` to `true` switches the CLI to registering
each listed `.psc` directly with the `FunctionTable`
(`FunctionTable::with_known_scripts` in `papyrus-lint-core`) instead,
scoping resolution strictly to what the achlist actually lists — cross-listed-directory
resolution still works, but nothing unlisted leaks in, and no directory is
scanned at all — with `script_locator::conflicting_script_versions_among`
covering the one case directory scanning otherwise catches for free: two
listed entries sharing a file name. It defaults to `false` so an existing
achlist-based project's resolution/diagnostics don't change underneath it.
Configuration controls formatting, lint enablement, complexity thresholds,
CLI failure levels, and the compiler path. It also controls whether the
desktop app's `lint_psc_file`/`repair_psc_file` commands additionally run
PapyrusCompiler.exe against a dropped `.psc` as part of linting it
(`compile_check`, off by default), merging in any errors it reports (see
`app/src-tauri/src/compile_diagnostics.rs`) alongside the lint engine's
own; unlike the "Compile"/"Save & Compile" buttons, this always compiles
into a throwaway temporary directory rather than the project's real
output directory. See the [README configuration
reference](README.md#configuration) for the per-key documentation, and
[`docs/papyrus-lint.default.yaml`](docs/papyrus-lint.default.yaml) — the
same file `PapyrusLinterCLI init` writes and the one the README links to
instead of dumping inline — for the complete default file. That file must
stay byte-for-byte identical to `PapyrusLinterCLI init`'s output (built
from `papyrus_lints::Config::default()` and the `FIELD_COMMENTS` table in
`papyrus-lint-core/src/config.rs`, which is what actually generates the
per-key comments): `papyrus-lint-core`'s
`config::tests::default_config_matches_the_checked_in_docs_copy` test
fails CI if they drift, so regenerate it with `PapyrusLinterCLI init`
(and update `FIELD_COMMENTS`/README together) whenever a default or a
field comment changes.

The desktop app's `parse_psc_file` command, and both the app's and the
CLI's cross-script lookups (`papyrus-lint-core`'s `function_table.rs`,
used to resolve the "Argument type check"/"Return type check" lints
across scripts), cache each parsed `.psc` AST on disk
(`app/crates/papyrus-lint-core/src/ast_cache.rs`), in an `ast-cache`
directory next to the running executable — the desktop app's own binary,
or `PapyrusLinterCLI`'s, whichever process is doing the parsing. A cached
entry is only reused when its stored MD5 of the file's content and the
file's last-modified timestamp still match, and the linter version that
wrote the entry is at or above a `MIN_COMPATIBLE_VERSION` constant
(currently `1.16.0`) rather than an exact match against the running
version — so an ordinary app update doesn't discard an otherwise
still-valid cache, and `MIN_COMPATIBLE_VERSION` only needs bumping when a
release actually changes the cache entry layout or the AST shape it
embeds. Any mismatch, or any I/O/(de)serialization failure reading the
cache, falls back to a fresh parse, so a stale or corrupt cache never
surfaces as a lint error. Since it lives in `papyrus-lint-core`, the same
cache backs the editor extensions too, which invoke `PapyrusLinterCLI` as
a subprocess.

Independent of that disk cache, `papyrus-parser`'s own `parse()` and
`tokenize()` entry points (`app/crates/papyrus-parser/src/cache.rs`) are
memoized in-memory against the most recently seen source string: a single
`papyrus_lints::lint()`/`lint_with_external_arguments()` pass over one
script calls into them dozens of times (each AST-based lint rule calls
`parse()`, each rule that works on raw tokens instead — e.g.
`chain_whitespace`, `exclamation_spacing`, `indentation` — calls
`tokenize()`) with the exact same source text, so a single-slot,
thread-local cache keyed by content equality turns all but the first call
into a clone instead of a re-lex/re-parse. This is deliberately simpler
than the disk-backed `ast_cache`: it never outlives the process (or even
the thread) and so needs no path, mtime, or version bookkeeping, since it
only ever has to remember the one source string a lint/repair pass is
currently working on.

## Keeping agent instructions synchronized

`AGENTS.md` and `CLAUDE.md` must contain the same project guidance. Whenever
one file is updated, make the equivalent update to the other file in the same
change and verify that the two files remain identical.

Whenever the documented lints in `README.md` are updated, make the corresponding
lint update to `docs/nexuspage.bbcode` in the same change. Preserve the Nexus page's
existing style: keep its lint descriptions shorter and more concise than the
README rather than copying the README's longer explanations verbatim. Whenever
the CLI usage examples/options or the documented default configuration in
`README.md` are updated, make the corresponding update to the CLI or
configuration section of `docs/nexuspage.bbcode` in the same change. Other README
changes do not need to be synchronized to the Nexus page.

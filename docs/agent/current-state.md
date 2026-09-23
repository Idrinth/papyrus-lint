<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Current state

This file is a **map**, not a second implementation. Behavior lives in
the code and tests cited below. Do not paste feature walkthroughs back
here; add a row or a one-line invariant instead.

Folder layout is in [`docs/project-structure.md`](../project-structure.md).
Rule metadata is generated from [`shared/rules/`](../../shared/rules)
(see `AGENTS.md`). How to run tests is in
[`development.md`](development.md).

## Invariants

These are the easy-to-miss contracts. If a change would violate one,
update the cited code *and* this list.

- `check`/`check_with` do not parse or tokenize themselves.
  `registry::collect_diagnostics` tokenizes and parses once and passes
  `Option<&[Token]>` / `Option<&Script>`. `repair()` re-parses after each
  fix. Exception: standalone `check_argument_types` still parses.
- Visitor rules (`visitor()` → `LintVisitor::Ast` or `Tokens`) walk once
  per pass. `none` rules run through `check` only.
- Reuse `const_eval` and `type_flow`; do not copy them into a new rule.
- Cross-script semantics go through `ExternalSignatures`
  (`external_signatures.rs`). Side-effect flags are computed in
  `papyrus-lint-core`, not re-derived in a lint.
- `lint` / `repair` / `repair_filtered*` have no resolver.
  `unused-import` is a no-op there. Callers with a project use the
  `*_with_external_arguments` siblings. Preview repair is resolver-less
  on purpose. Line-count-shifting fixes (`unused-import`,
  `property-sorting`) are rejected by per-line fix.
- `--blob` is in-memory only: no project root, no `FunctionTable`, no
  fix. Editors use it for unsaved buffers; saved files use the full CLI.
- `doc_url` is built in `papyrus-lint-output`. Do not invent a second
  URL scheme. AI export schemas are versioned under `schema/`; old
  versions stay frozen.
- `configuration/papyrus-lint.default.yaml` must match `init` output.
  Drift is a CI failure in `papyrus-lint-config`.
- Filesystem Tauri commands that parse, lint, repair, or compile are
  `#[tauri::command(async)]`. Only instant in-memory commands stay sync.
- On-disk AST cache filenames are namespaced by target game and entries are
  keyed by content MD5 + mtime +
  `MIN_COMPATIBLE_VERSION`. Bump that floor only when the entry layout
  or embedded AST changes, and update `schema/ast-cache-entry.schema.json`.
- A `.ppj`'s own `<Import>` entries (`ppj::PpjProject::imports`) feed
  `additional_script_roots` for that run/`init` — never
  `lookup_script_roots`. `ppj::parse_ppj` normalizes `\` to `/` before
  resolving a path (a ppj is Windows-authored) but never decomposes an
  already-absolute Windows/UNC path into components; the CLI's `.ppj`
  handling lives in `run_scan.rs`/`doctor/checks.rs`/`init.rs`, not
  `papyrus-lint-core`, which only parses the file. The GUI's own `.ppj`
  drop mode (`parse_ppj_file` in `app/src-tauri/src/files.rs`) mirrors
  this: its `<Import>` entries land in `currentPpjImportRoots`
  (`project-state.ts`), folded into `effectiveScriptRoots()` the same way
  `currentAchlistScriptRoots` is.
- Vanilla engine types without an on-disk `.psc` resolve from the bundled
  AST cache by `ScriptName` (`FunctionTable::ensure_loaded` /
  `script_exists`). A project or lookup-root file of the same name still
  wins.

## Where to read

| If you are changing… | Open |
| --- | --- |
| Parser / AST / lexer / in-memory memo | `app/crates/papyrus-parser/src/` (`ast.rs`, `parser.rs`, `lexer.rs`, `cache.rs`, `types.rs`) |
| Disk AST/token cache, bundled vanilla scripts | `app/crates/papyrus-ast-cache/src/` |
| Rule dispatch, visitors, tags, disable comments | `app/crates/papyrus-lints/src/` (`lib.rs`, generated `registry`/`tags`/`config`, `external_signatures.rs`, `const_eval.rs`) |
| A single rule | `app/crates/papyrus-lints/src/<rule>.rs` + `shared/rules/<id>.json` |
| Project root, achlist/ppj, script index, FunctionTable, compile/stale `.pex` | `app/crates/papyrus-lint-core/src/` |
| `papyrus-lint.yaml`, presets, compiler/game-install detection | `app/crates/papyrus-lint-config/src/` |
| CLI (`run`, `run_blob`, `fix`, `doctor`, `--tag`) | `app/crates/papyrus-lint-cli/src/` |
| Text / JSON / AI report formatting | `app/crates/papyrus-lint-output/` and `schema/` |
| Tauri commands | `app/src-tauri/src/` (`files.rs`, `lint.rs`, `repair.rs`, `export.rs`, `lint_config.rs`) |
| Desktop UI (drop, results, live edit, watch, presets) | `app/src/` (`drop.ts`, `results-filter.ts`, `live-edit.ts`, `watch.ts`, `presets.ts`) |
| VS Code live lint / ignore | `vscode-extension/src/liveLint.ts`, `linter.ts`, `ignore.ts` |
| Sublime unsaved-buffer lint | `SublimeLinter-contrib-papyrus-lint/linter.py` |

## Surfaces that must stay aligned

A behavior change in the engine often needs a matching caller. Check
the other column before calling the work done.

| Engine piece | Also used by |
| --- | --- |
| `papyrus_lints::lint` / `repair` | CLI, Tauri, editor plugins (via CLI) |
| `*_with_external_arguments` | CLI `fix` and Tauri apply-fix commands — not preview |
| `ExternalSignatures` / `FunctionTable` | CLI threads (`SharedFunctionTable`), desktop per-project table |
| `find_candidate_pair_root` / script locator | CLI path resolution, Tauri `find_project_root`, drop-folder scan |
| `strict_achlist_scope` / `lookup_script_roots` | CLI + config; lookup roots are analysis-only (never linted, never on compiler `-i`) |
| Rule tags / `doc_url` | CLI reports, GUI badges/filters, VS Code diagnostic code, Sublime message text |
| Presets / executable-adjacent base config | `init --preset`, GUI picker / Presets tab |

## Do not put here

- Per-rule descriptions — `shared/rules/<id>.json` and `rules.html`.
- CLI flag lists and config keys — `docs/cli.md`, `docs/configuration.md`.
- CI steps — comments in the related workflow; Pages / release steps —
  `pages.md`, `releases.md`.
- File-by-file tree — `docs/project-structure.md`.
- Historical "unlike before" narrative. If the code changed, delete the
  story; keep the invariant.

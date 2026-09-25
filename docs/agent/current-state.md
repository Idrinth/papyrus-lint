<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Current state

This file is a **map** of contracts that cross a crate or a UI surface.
Crate-local behavior, and how to test that crate, live in its `README.md`
under `app/crates/` (desktop shell: `app/src-tauri/README.md`). Do not
paste feature walkthroughs back here; add a row or a one-line invariant
instead.

Folder layout is in [`docs/project-structure.md`](../project-structure.md).
Rule metadata is generated from [`shared/rules/`](../../shared/rules)
(see `AGENTS.md`). How to run the desktop app, frontend, and editor
plugins is in [`development.md`](development.md).

Latest published release is `v2.1.0`. VS Code and Sublime still drive
analysis through `PapyrusLinterCLI`; they do not host the LSP process.
The server itself is
[`papyrus-lint-lsp`](../../app/crates/papyrus-lint-lsp/README.md).

## Invariants

If a change would violate one, update the cited code *and* this list.
Anything that belongs to one crate belongs in that crate's README.

- The desktop Settings tab, the first-run picker, and the VS Code /
  Sublime `init` prompts write `game` as `skyrim` or `fallout4`.
  Starfield stays CLI-only (`init --game starfield`) until the linter
  supports it.
- The GUI `.ppj` drop mode (`parse_ppj_file` in
  `app/src-tauri/src/files.rs`) puts `<Import>` entries in
  `currentPpjImportRoots` (`app/src/project-state.ts`), folded into
  `effectiveScriptRoots()` the same way `currentAchlistScriptRoots` is.
  Never `lookup_script_roots`. Parsing rules are in the
  [`papyrus-lint-core`](../../app/crates/papyrus-lint-core/README.md)
  README.

## Where to read

| If you are changing… | Open |
| --- | --- |
| A reusable crate | That crate's `README.md` under `app/crates/` |
| Tauri commands | [`app/src-tauri/README.md`](../../app/src-tauri/README.md) |
| Desktop UI (drop, results, live edit, watch, presets) | `app/src/` (`drop.ts`, `results-filter.ts`, `live-edit.ts`, `watch.ts`, `presets.ts`) |
| VS Code CLI-backed lint / ignore / actions | `vscode-extension/src/` (`liveLint.ts`, `linter.ts`, `ignore.ts`, `codeActions.ts`, `suppressions.ts`) |
| Sublime unsaved-buffer lint | `SublimeLinter-contrib-papyrus-lint/linter.py` |

## Surfaces that must stay aligned

A behavior change in the engine often needs a matching caller. Check
the other column before calling the work done.

| Engine piece | Also used by |
| --- | --- |
| `papyrus_lints::lint` / `repair` | `papyrus-lint-live` (CLI `--blob`, LSP snapshot, desktop live edit), editor plugins (via CLI), project-aware CLI / Tauri file lint |
| `*_with_external_arguments` | CLI `fix` and Tauri apply-fix commands — not preview |
| `ExternalSignatures` / `FunctionTable` | CLI threads (`SharedFunctionTable`), desktop per-project table |
| `find_candidate_pair_root` / script locator | CLI path resolution, Tauri `find_project_root`, drop-folder scan |
| `strict_achlist_scope` / `lookup_script_roots` | CLI + config; lookup roots are analysis-only (never linted, never on compiler `-i`) |
| Rule tags / `doc_url` | CLI reports, GUI badges/filters, VS Code diagnostic code, Sublime message text, LSP diagnostic code |
| Presets / executable-adjacent base config | `init --preset`, GUI picker / Presets tab |
| Automatic-fix edit application | Tauri apply-fix, CLI `fix`, VS Code / Sublime CLI wrappers, LSP code actions and `papyrusLint.fixFile` |

## Do not put here

- Crate-local invariants or `cargo` commands — that crate's `README.md`.
- Per-rule descriptions — `shared/rules/<id>.json` and `rules.html`.
- CLI flag lists and config keys — `docs/cli.md`, `docs/configuration.md`.
- CI, Pages, and release steps — comments in the related workflow.
- File-by-file tree — `docs/project-structure.md`.
- Historical "unlike before" narrative. If the code changed, delete the
  story; keep the invariant.

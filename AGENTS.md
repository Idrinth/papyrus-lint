# Papyrus Lint

A linter for Bethesda's Papyrus scripting language, shipped as a Tauri
desktop app, a standalone CLI (`PapyrusLinterCLI`), and VS Code /
SublimeLinter plugins. Default branch is `the-one` (not `main`).

This file is the **always-on** agent index. Keep it short. Deep notes live
in [`docs/agent/`](docs/agent/) and are loaded only when the task needs
them. `CLAUDE.md` is a pointer here, not a second copy.

## Read on demand

| If you are changing… | Read |
| --- | --- |
| Anything, first time in this repo | This file, then [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Parser, a lint rule, CLI, GUI, or editor plugin | [`docs/agent/current-state.md`](docs/agent/current-state.md) |
| Crate / folder layout | [`docs/agent/project-structure.md`](docs/agent/project-structure.md) |
| How to run tests or the desktop app | [`docs/agent/development.md`](docs/agent/development.md) |
| `.github/workflows/ci.yml` or CI scripts | [`docs/agent/ci.md`](docs/agent/ci.md) |
| `pages/` or the GitHub Pages workflow | [`docs/agent/pages.md`](docs/agent/pages.md) |
| `.github/workflows/release.yml` | [`docs/agent/releases.md`](docs/agent/releases.md) |
| Lint descriptions / rule docs | [`README.md`](README.md#implemented-lints) |

Do not paste those files back into this index. Update the file you read.

## Crate map

| Crate | Path | Role |
| --- | --- | --- |
| `papyrus-parser` | `app/crates/papyrus-parser` | Lexer, AST, parser. No lint rules. |
| `papyrus-lints` | `app/crates/papyrus-lints` | Rules, `lint()` / `repair()`, config, tags. |
| `papyrus-lint-config` | `app/crates/papyrus-lint-config` | Locates/loads/saves a project's `papyrus-lint.yaml`, presets. |
| `papyrus-lint-core` | `app/crates/papyrus-lint-core` | Project root, achlist, function table, compiler. |
| `papyrus-lint-cli` | `app/crates/papyrus-lint-cli` | `PapyrusLinterCLI`. Shared `run()` used by the desktop binary too. |
| desktop shell | `app/src-tauri` | Tauri commands + GUI/CLI dispatch. |
| frontend | `app/src` | Vanilla TypeScript. No framework. |
| VS Code | `vscode-extension/` | Editor integration. |
| Sublime | `SublimeLinter-contrib-papyrus-lint/` | Editor integration. |
| rule data | `rules/*.yaml` | Compiled in by `papyrus-lints` / `papyrus-lint-core` `build.rs`. |

The five reusable crates are **path dependencies, not Cargo workspace
members**. Run `cargo test` / `cargo fmt` / `cargo clippy` against each
crate's own `Cargo.toml`. Only `app/src-tauri` needs Tauri system deps.

## Commands

From the repo root, typical loops:

- Parser: `cargo test --manifest-path app/crates/papyrus-parser/Cargo.toml`
- Lints: `cargo test --manifest-path app/crates/papyrus-lints/Cargo.toml`
- Config: `cargo test --manifest-path app/crates/papyrus-lint-config/Cargo.toml`
- Core: `cargo test --manifest-path app/crates/papyrus-lint-core/Cargo.toml`
- CLI: `cargo test --manifest-path app/crates/papyrus-lint-cli/Cargo.toml`
- Frontend (`app/`): `npm test`, `npm run lint`, `npm run build`
- VS Code (`vscode-extension/`): `npm test`, `npm run lint`, `npm run compile`
- Sublime: `python -m unittest discover -s SublimeLinter-contrib-papyrus-lint/tests -v`

Rust format/lint: `cargo fmt --check --manifest-path <crate>/Cargo.toml`
and `cargo clippy --manifest-path <crate>/Cargo.toml --all-targets -- -D warnings`.
CI treats clippy warnings as errors.

## Hard rules

1. **PR labels.** Every pull request needs at least one `component: ...`
   label and at least one `type: ...` label. CI's `labels` job fails the
   rest of CI without them. Components: `sublime lint plugin`, `vscode
   extension`, `frontend`, `linting`, `ci`, `parsing`, `documentation`,
   `pages`, `gui`, `cli`. Types: `breaking change`, `feature`,
   `refactoring`, `tests`, `documentation`, `dependency` (add a new
   `type:` if none fit).
2. **Stay on `the-one`.** Rebase/merge `the-one` into a PR branch before
   asking for merge.
3. **Do not duplicate agent docs.** Edit `AGENTS.md` (this index) or a
   file under `docs/agent/`. `CLAUDE.md` must remain a pointer to this
   file, not a copy of it.
4. **Lint descriptions have three consumers.** A README Implemented Lints
   row is the source of truth. The same change must update
   `app/crates/papyrus-lints/src/tags.rs` (`description` + `doc_slug`) and
   a shorter blurb in `docs/nexuspage.bbcode`. `pages/build.py` generates
   the website tables from the README, so the site does not need a manual
   lint-table edit.
5. **Match the file you are in.** Don't invent a new module layout, naming
   scheme, or comment style in a file that already has one.
6. **Don't gold-plate.** A bug fix does not need a surrounding refactor.
   A new rule does not need a new abstraction for "all future rules".

## Adding a lint

Minimum touch list (see also [`CONTRIBUTING.md`](CONTRIBUTING.md)):

1. `app/crates/papyrus-lints/src/<rule>.rs` — check (and optional repair)
   plus tests.
2. `app/crates/papyrus-lints/src/lib.rs` — `pub mod`, `KNOWN_RULE_IDS`,
   `lint_with_external_arguments_and_extra_diagnostics` dispatch, and
   `FIXABLE_RULE_IDS` / repair dispatch if it auto-fixes.
3. `app/crates/papyrus-lints/src/config.rs` — field on `Rules` and its
   `Default` (and the rustdoc yaml example at the top of the file).
4. `app/crates/papyrus-lints/src/tags.rs` — `RULE_TAGS` entry: `rule`,
   `doc_slug` (from the README row title, slugified), `description`
   (verbatim README cell), `kinds`, `importance`.
5. `README.md` Implemented Lints table.
6. `docs/nexuspage.bbcode` — shorter description, same style as the
   surrounding rows.

Rules should inspect source/tokens so they still run on scripts that
don't parse. Configurable behavior goes on `&papyrus_lints::Config`, not
a new extra parameter. Tests should cover diagnostics, `@disable` /
`@disable-file`, config off-switches, and repairs when those apply.

If the rule introduces a new *kind* keyword (not `style` /
`performance` / `correctness` / `maintainability`), also update
`TAG_KINDS` in `app/src/main.ts`.

## Docs sync (humans and AI)

- README lint tables → `tags.rs` + `docs/nexuspage.bbcode` (rule 4).
- README CLI usage / default config → `docs/nexuspage.bbcode` CLI or
  configuration section. Other README edits do not need a Nexus update.
- `pages/index.template.html` lint tables and CLI examples are generated
  from the README on deploy; only its hand-authored hero/cards/blurbs
  need a human pass when those parts of the README change meaningfully.

## Pull request body

Use `.github/pull_request_template.md`. Keep the change focused. Do not
open a PR until the crate(s) you touched pass the commands above.

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
| `papyrus-ast-cache` | `app/crates/papyrus-ast-cache` | Disk-backed AST/token cache, keyed by content MD5 + mtime + linter version. Re-exported by `papyrus-lint-core` as `ast_cache`. |
| `papyrus-lints` | `app/crates/papyrus-lints` | Rules, `lint()` / `repair()`, config, tags. |
| `papyrus-lint-config` | `app/crates/papyrus-lint-config` | Locates/loads/saves a project's `papyrus-lint.yaml`, presets. |
| `papyrus-lint-core` | `app/crates/papyrus-lint-core` | Project root, achlist, function table, compiler. |
| `papyrus-lint-cli` | `app/crates/papyrus-lint-cli` | `PapyrusLinterCLI`. Shared `run()` used by the desktop binary too. |
| `papyrus-lint-output` | `app/crates/papyrus-lint-output` | Plain-text/JSON/AI-export report formatting, shared by `papyrus-lint-cli` and the desktop app's Tauri commands (`app/src-tauri/src/export.rs`). |
| desktop shell | `app/src-tauri` | Tauri commands + GUI/CLI dispatch. |
| frontend | `app/src` | Vanilla TypeScript. No framework. |
| VS Code | `vscode-extension/` | Editor integration. |
| Sublime | `SublimeLinter-contrib-papyrus-lint/` | Editor integration. |
| rule data | `rules/*.yaml`, `docs/rules.json` | Compiled in by `papyrus-lints` / `papyrus-lint-core` `build.rs`; `docs/rules.json`'s `importance`/`kept_in_standard` also drive `papyrus-lint-config/build.rs`'s generated `standard`/`careful` presets. |

The seven reusable crates are **path dependencies, not Cargo workspace
members**. Run `cargo test` / `cargo fmt` / `cargo clippy` against each
crate's own `Cargo.toml`. Only `app/src-tauri` needs Tauri system deps.

## Commands

From the repo root, typical loops:

- Parser: `cargo test --manifest-path app/crates/papyrus-parser/Cargo.toml`
- AST cache: `cargo test --manifest-path app/crates/papyrus-ast-cache/Cargo.toml`
- Lints: `cargo test --manifest-path app/crates/papyrus-lints/Cargo.toml`
- Config: `cargo test --manifest-path app/crates/papyrus-lint-config/Cargo.toml`
- Core: `cargo test --manifest-path app/crates/papyrus-lint-core/Cargo.toml`
- Output formatting: `cargo test --manifest-path app/crates/papyrus-lint-output/Cargo.toml`
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
4. **`docs/rules.json` is the single source of truth for lint metadata.**
   A rule's entry there (`id`, `name`, `definition` — the long text,
   `description` — a shorter blurb matching `docs/nexuspage.bbcode`'s own
   style, `category`, `tags`, `severity`, `importance`, `fixable`) is what
   every other consumer generates from. Nothing else is hand-edited from
   it: `build.rs` compiles `app/crates/papyrus-lints`'s
   `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS` (`src/registry.rs`), `RULE_TAGS`
   (`src/tags.rs`), `Rules`/`default_rules()` (`src/config.rs`), and the
   `collect_diagnostics`/`apply_repairs` dispatch (from `rule_dispatch.json`)
   from it at build time; `pages/build.py` generates the
   website's searchable `rules.html` straight from it; and release tooling
   fills in `docs/nexuspage.bbcode`'s five lint tables from it (see
   Releases in `docs/agent/releases.md`) — the checked-in file carries no
   rows itself. `README.md`'s own
   "Implemented Lints" section only keeps a short per-category blurb and a
   link to `rules.html` — it carries no per-rule text to keep in sync.
5. **Match the file you are in.** Don't invent a new module layout, naming
   scheme, or comment style in a file that already has one.
6. **Don't gold-plate.** A bug fix does not need a surrounding refactor.
   A new rule does not need a new abstraction for "all future rules".
7. **AI-authored PRs get a model label.** In addition to the
   `component:`/`type:` labels, tag the PR with a label naming the model
   that wrote it — whatever it is actually called, e.g. `codex`, `grok`,
   `Claude Sonnet 5`. Create the label if it doesn't exist yet.

## Adding a lint

Minimum touch list (see also [`CONTRIBUTING.md`](CONTRIBUTING.md)):

1. `app/crates/papyrus-lints/src/<rule>.rs` — check (and optional repair);
   put its tests in a sibling `<rule>_tests.rs`, included via
   `#[cfg(test)] #[path = "<rule>_tests.rs"] mod tests;`.
2. `app/crates/papyrus-lints/src/lib.rs` — `mod`.
3. `app/crates/papyrus-lints/rule_dispatch.json` — `check` (a Rust
   expression, e.g. `trailing_whitespace::check(source)`). If the rule
   auto-fixes inside `registry::apply_repairs`, also set `repair` and
   `repair_order`. Project-level rules and `unused-disable` omit `check`
   (they run outside this crate / after the main pass). `Rules`,
   `default_rules()`, `collect_diagnostics`, and `apply_repairs` are
   generated from this file plus `docs/rules.json` — don't hand-edit
   them.
4. `docs/rules.json` — a new entry: `id`, `name`, `definition` (the long,
   README-style description), a short `description` blurb matching
   `docs/nexuspage.bbcode`'s style, `category` (one of `Formatting`,
   `Performance`, `Reliability`, `Bugprone`, `Other`), `tags`,
   `importance`, `severity`, and `fixable`. Set `"enabled_by_default":
   false` only for opt-in rules. No Nexus page regeneration step
   is needed here — that happens at release time (see Releases in
   `docs/agent/releases.md`). `build.rs` generates
   `registry.rs`'s `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS`, `tags.rs`'s
   `RULE_TAGS`, `config.rs`'s `Rules`/`default_rules()`, and
   `collect_diagnostics`/`apply_repairs` from this file (and
   `rule_dispatch.json`) at build time — don't hand-edit those;
   `doc_url()` links straight to `rules.html#rule-<rule>`, derived from
   the rule id alone, so it needs no separate slug field either. A new
   `"low"` importance rule is turned off by default in the generated
   `standard`/`careful` presets too (see `papyrus-lint-config/build.rs`);
   add `"kept_in_standard": true` to its entry only if it belongs with the
   handful of cheap, auto-fixable formatting rules `standard` keeps on
   regardless.
5. `README.md`'s "Implemented Lints" section — add a one-line mention
   under the matching category blurb only if the category's own summary
   no longer describes what the new rule does; the per-rule reference
   lives on `rules.html`, not in the README.

Rules should inspect source/tokens so they still run on scripts that
don't parse. Configurable behavior goes on `&papyrus_lints::Config`, not
a new extra parameter. Tests should cover diagnostics, `@disable` /
`@disable-file`, config off-switches, and repairs when those apply.

If the rule introduces a new *kind* keyword (not `style` /
`performance` / `correctness` / `maintainability`), also update
`TAG_KINDS` in `app/src/main.ts`.

## Docs sync (humans and AI)

- README lint tables → `docs/rules.json` (rule 4). `docs/rules.json` →
  `docs/nexuspage.bbcode`'s lint tables (filled in at release time, never
  checked in — see Releases in `docs/agent/releases.md`) and
  `papyrus-lints`'s `registry.rs`/`tags.rs` (via `build.rs`) — all
  generated, never hand-edited.
- README CLI usage / default config → `docs/nexuspage.bbcode` CLI or
  configuration section (hand-edited; not covered by the generator
  above). Other README edits do not need a Nexus update.
- `pages/index.template.html` lint tables and CLI examples are generated
  from the README on deploy; only its hand-authored hero/cards/blurbs
  need a human pass when those parts of the README change meaningfully.

## Pull request body

Use `.github/pull_request_template.md`. Keep the change focused. Do not
open a PR until the crate(s) you touched pass the commands above.

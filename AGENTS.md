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
| Crate / folder layout | [`docs/project-structure.md`](docs/project-structure.md) |
| How to run tests or the desktop app | [`docs/agent/development.md`](docs/agent/development.md) |
| `.github/workflows/ci.*.yml` or CI scripts | The explanatory comments in the related workflow |
| `pages/` or the GitHub Pages workflow | [`docs/agent/pages.md`](docs/agent/pages.md) |
| `.github/workflows/release.yml` | [`docs/agent/releases.md`](docs/agent/releases.md) |
| Lint descriptions / rule docs | [`README.md`](README.md#implemented-lints) |

Do not paste those files back into this index. Update the file you read.

## Crate map

The eight reusable linting crates under `app/crates/` are **path dependencies, not
Cargo workspace members**. Run `cargo test` / `cargo fmt` / `cargo clippy`
against each crate's own `Cargo.toml`. Only `app/src-tauri` needs Tauri
system deps. Paths and roles live in
[`docs/project-structure.md`](docs/project-structure.md) — do not recopy
that table here.

## Commands

From the repo root, typical loops:

- Parser: `cargo test --manifest-path app/crates/papyrus-parser/Cargo.toml`
- AST cache: `cargo test --manifest-path app/crates/papyrus-ast-cache/Cargo.toml`
- Collision cache: `cargo test --manifest-path app/crates/papyrus-collision-cache/Cargo.toml`
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
4. **`shared/rules/<id>.json` is the single source of truth for lint
   metadata.** Each rule is one file there (one JSON object: `id`, `name`,
   `definition` — the long text, `description` — a shorter blurb matching
   `templates/nexuspage.bbcode`'s own style, `category`, `visitor` (`ast`,
   `tokens`, or `none` — how the rule would walk a script as a visitor),
   `tags`, `severity`,
   `importance`, `fixable`, optional `repair_order`). `shared/rules.json`
   — the combined array every other consumer actually reads — is
   generated from those files by `.github/scripts/build_rules_json.py`
   and is git-ignored, not checked in; run that script (no arguments)
   after adding/editing a `shared/rules/*.json` file and before building
   or testing anything below. Nothing else is hand-edited from it:
   `build.rs` compiles `app/crates/papyrus-lints`'s
   `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS` (`src/registry.rs`), `RULE_TAGS`
   (`src/tags.rs`), `Rules`/`default_rules()` (`src/config.rs`), the
   `collect_diagnostics`/`apply_repairs` dispatch, and each rule's `mod`
   in `src/lib.rs` from the generated `shared/rules.json` at build time;
   `pages/build.py` generates the website's searchable `rules.html`
   straight from it; and release tooling fills in `templates/nexuspage.bbcode`'s
   five lint tables from it (see Releases in `docs/agent/releases.md`) —
   the checked-in `templates/nexuspage.bbcode` carries no rows itself. `README.md`'s own
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

1. `app/crates/papyrus-lints/src/<rule>.rs` — `check(source, ast, tokens,
   config, external)` (and optional `repair(source, ast, tokens, config)`);
   put its tests in a sibling `<rule>_tests.rs`, included via
   `#[cfg(test)] #[path = "<rule>_tests.rs"] mod tests;`. `build.rs`
   generates the `mod` in `src/lib.rs` from the `shared/rules/<id>.json`
   entry below.
2. `shared/rules/<id>.json` — a new file, named after the rule's `id`,
   holding a single object: `id`, `name`, `definition` (the long,
   README-style description), a short `description` blurb matching
   `templates/nexuspage.bbcode`'s style, `category` (one of `Formatting`,
   `Performance`, `Reliability`, `Bugprone`, `Other`), `visitor` (`ast`
   for a walk of parsed nodes, `tokens` for a walk of the lexer stream,
   `none` for a project-level/post-pass/raw-line rule that is neither),
   `tags`,
   `importance`, `severity`, and `fixable`. For an `apply_repairs` auto-fix,
   set `repair_order` (1..=N, no gaps). Set `"enabled_by_default":
   false` only for opt-in rules. Run `python3
   .github/scripts/build_rules_json.py` afterward (and before building or
   testing anything below) to regenerate the git-ignored `shared/rules.json`
   every consumer below actually reads. No Nexus page regeneration step
   is needed here — that happens at release time (see Releases in
   `docs/agent/releases.md`). `build.rs` generates
   `registry.rs`'s `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS`, `tags.rs`'s
   `RULE_TAGS`, `config.rs`'s `Rules`/`default_rules()`,
   `collect_diagnostics`/`apply_repairs`, and `lib.rs`'s rule `mod`s from
   this file at build time — don't hand-edit those;
   `doc_url()` links straight to `rules.html#rule-<rule>`, derived from
   the rule id alone, so it needs no separate slug field either. A new
   `"low"` importance rule is turned off by default in the generated
   `standard`/`careful` presets too (see `papyrus-lint-config/build.rs`);
   add `"kept_in_standard": true` to its entry only if it belongs with the
   handful of cheap, auto-fixable formatting rules `standard` keeps on
   regardless.
3. `README.md`'s "Implemented Lints" section — add a one-line mention
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

- README lint tables → `shared/rules/<id>.json` (rule 4). `shared/rules/*.json` →
  the generated `shared/rules.json` (`.github/scripts/build_rules_json.py`,
  also git-ignored) → `templates/nexuspage.bbcode`'s lint tables (filled in at
  release time, never checked in — see Releases in
  `docs/agent/releases.md`) and `papyrus-lints`'s `registry.rs`/`tags.rs`/
  `lib.rs` rule `mod`s (via `build.rs`) — all generated, never hand-edited.
- `docs/cli.md`/`docs/configuration.md` CLI usage / default config →
  `templates/nexuspage.bbcode` CLI or configuration section (hand-edited; not
  covered by the generator above). Other README/`docs/*.md` edits do not
  need a Nexus update.
- `CONTRIBUTING.md` development setup and this index's crate map are
  pointers, not copies: edit [`docs/agent/development.md`](docs/agent/development.md)
  or [`docs/project-structure.md`](docs/project-structure.md) instead of
  pasting those files back here.
- `pages/index.template.html`'s CLI examples and every `docs/` subpage are
  generated from `README.md`/`docs/*.md` on deploy — see Pages in
  `docs/agent/pages.md` for the `<!--CLI_EXAMPLES-->` extraction and the
  `DOCS` list a new `docs/*.md` file needs an entry in.
- Contact / download / documentation URLs → `shared/links.yaml`. Markers
  (`<!--CONTACT-LINKS-->`, `<CONTACT-LINKS>`, `<LINKS>`) are filled at
  build time by the tag in the marker; never name a YAML key at a
  destination.

## Pull request body

Use `.github/pull_request_template.md`. Keep the change focused. Do not
open a PR until the crate(s) you touched pass the commands above.

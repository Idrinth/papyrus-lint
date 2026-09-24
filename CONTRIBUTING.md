# Contributing to Papyrus Lint

Thanks for your interest in contributing! This document covers how the
project is laid out, how to set up a development environment, and what's
expected of a pull request.

## Project structure

The canonical, detailed repository layout lives in
[`docs/project-structure.md`](docs/project-structure.md). It is shared with
coding agents so contributors only have one project tree to keep current.

## Development setup

How to install, test, and run each crate, the desktop app, and the editor
plugins lives in [`docs/agent/development.md`](docs/agent/development.md)
(including generating `shared/rules.json` after clone, and Tauri's
platform prerequisites for a desktop build). Do not recopy that command
list here.

## Before opening a pull request

CI (`.github/workflows/ci.*.yml`) runs on every pull request and on pushes to
`the-one` (the default branch — not `main`). Make sure your change passes
the same checks locally first:

- **Sublime Text extension job**: runs `python -m unittest discover -s
  SublimeLinter-contrib-papyrus-lint/tests -v`.
- **Frontend job**: from `app/`, `npm ci`, then `npm run lint` (ESLint), `npm
  run test:coverage` (Vitest unit tests, instrumented for coverage), and `npm
  run build` (typecheck & Vite build).
- **Rust build job**, against `app/src-tauri/Cargo.toml`:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo check`
- **VS Code extension job**: from `vscode-extension/`, runs `npm test`,
  `npm run lint`, and `npm run compile`.
- **Rust test job**: a matrix over `app/src-tauri`, `app/crates/papyrus-parser`,
  `app/crates/papyrus-ast-cache`, `app/crates/papyrus-collision-cache`,
  `app/crates/papyrus-lints`,
  `app/crates/papyrus-lint-config`, `app/crates/papyrus-lint-core`,
  `app/crates/papyrus-lint-output`, `app/crates/papyrus-lint-lsp`, and
  `app/crates/papyrus-lint-cli` runs each crate's tests via `cargo llvm-cov`.
  If you touched any of those crates, run `cargo test` (or `cargo
  llvm-cov`, to also see coverage — see the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) docs for
  setup) from that crate's directory to make sure its suite still passes.

Keep pull requests focused on a single change, and use the PR template's
checklist — contributions are permanent and unpaid; only open a PR once
you're comfortable with both.

Every pull request must also be tagged with at least one label naming the
component(s) it affects:

- `component: sublime lint plugin`
- `component: vscode extension`
- `component: frontend`
- `component: linting`
- `component: ci`
- `component: parsing`
- `component: documentation`
- `component: pages`
- `component: gui`
- `component: cli`

The release workflow groups release notes by these labels instead of
listing merged pull requests flat, so an unlabeled (or mislabeled) pull
request falls into a trailing "Other" section of the release notes
rather than under its actual component.

Every pull request must also carry at least one `type: ...` label (e.g.
`type: feature`, `type: documentation`) naming the kind of change it
makes; unlike the component labels above, this set isn't fixed, so add a
new `type: ...` label if a pull request doesn't fit an existing one. CI's
`labels` job fails before running the rest of CI if either requirement
is missing.

Before merging (or asking a maintainer to merge) a pull request, make sure
its branch is up to date with `the-one`. Merge or rebase `the-one` into the
branch first if it has fallen behind, so CI has run against the current
base.

## Code style

- Rust code is formatted with `cargo fmt` and linted with `clippy`
  (warnings are treated as errors in CI). Run both before committing.
- TypeScript is checked with `tsc` as part of `npm run build`, and linted
  with ESLint (`npm run lint`) using `typescript-eslint`'s recommended
  rules plus `@vitest/eslint-plugin`'s recommended rules on test files.
- Match the existing style of the file you're editing (naming, module
  layout, etc.) rather than introducing a new convention.

## Adding lint rules

Lint rules live in `app/crates/papyrus-lints/src`; the complete current set and
each rule's behavior are documented one file per rule under
[`shared/rules/`](shared/rules), also browsable as the website's [full lint
rule reference](https://papyrus-lint.idrinth.de/rules.html). Rules generally
inspect raw source or lexer tokens so they keep running on scripts that do not
parse cleanly. Follow that approach for a new rule where practical, then add
`shared/rules/<id>.json` (`visitor`: `ast` / `tokens` / `none` for how the
rule would walk a script as a visitor; `repair_order` if `registry::apply_repairs` should
auto-fix it). `build.rs` generates the `mod` in
`app/crates/papyrus-lints/src/lib.rs` from that entry. Every
source-level check is `check(source, ast, tokens, config, external)` and
every `apply_repairs` fix is `repair(source, ast, tokens, config)`. A rule
that would walk the AST or token stream also exposes `visitor() ->
crate::visitor::LintVisitor`. That visitor owns a `Store` filled during the
walk. `collect_diagnostics` registers those onto the AST/token walkers,
walks both trees once, and drains the stores. `check` for those rules is
that same walk for a single visitor. `Rules`,
`default_rules()`, `collect_diagnostics`, `apply_repairs`,
`KNOWN_RULE_IDS`, `FIXABLE_RULE_IDS`, `RULE_TAGS`, and the rule `mod`s are
generated by `build.rs` — do not hand-edit them. Set `"enabled_by_default":
false` on the `shared/rules/<id>.json` entry for opt-in rules.

After adding or editing a `shared/rules/*.json` file, run `python3
.github/scripts/build_rules_json.py` to regenerate the git-ignored
`shared/rules.json` those generated files (and `pages/build.py`) actually
read — do this before building or testing anything below.

A lint/fix job receives a `&papyrus_lints::Config`, deserialized from a
project's optional `papyrus-lint.yaml`/`.yml`, so user-configurable behavior
should be read from there rather than added as a separate parameter. Add tests
for diagnostics, disable comments, configuration, and repairs as applicable,
and update `shared/rules/<id>.json` and the configuration
examples (`configuration/papyrus-lint.default.yaml`, `templates/nexuspage.bbcode`).
`registry.rs`'s `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS`, `tags.rs`'s
`RULE_TAGS`, `config.rs`'s `Rules`, the check/repair dispatch, and
`lib.rs`'s rule `mod`s are all compiled from the generated
`shared/rules.json` by `build.rs`, so they never need hand-editing.

## Reporting bugs and requesting features

Please use GitHub Issues. Include steps to reproduce for bugs (ideally a
minimal `.psc`/`.achlist` sample), and your OS/environment for anything
related to the desktop app.

## Code of Conduct

This project follows the [Contributor Covenant Code of
Conduct](CODE_OF_CONDUCT.md). By participating, you're expected to uphold
it.

## License

By contributing, you agree that your contributions will be licensed under
the project's [MIT License](LICENSE).

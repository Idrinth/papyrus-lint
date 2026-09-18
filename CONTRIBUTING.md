# Contributing to Papyrus Lint

Thanks for your interest in contributing! This document covers how the
project is laid out, how to set up a development environment, and what's
expected of a pull request.

## Project structure

```text
.
├── app/                     # The desktop app: Tauri (Rust + TypeScript) shell
│   │                        # and its frontend, with their npm/cargo config
│   ├── src/                  # Frontend (TypeScript, vanilla, no framework)
│   │   ├── main.ts              # Drag-and-drop UI façade: types, project/config
│   │   │                        # wiring, drop/lint orchestration; re-exports the
│   │   │                        # feature modules below
│   │   ├── presets.ts           # Config presets: picker, save/reset, Presets tab
│   │   ├── code-viewer.ts       # Code viewer dialog: open/close, view, line fix/ignore
│   │   ├── results-list.ts      # Lint results list, mass-fix; calls filter + export helpers
│   │   ├── results-filter.ts    # Lint results filters: state, matching, filterOutcomes
│   │   ├── results-export-types.ts # Shared types for issue exports (filters, files, AI source)
│   │   ├── results-export-text.ts  # Plain-text issue export formatter
│   │   ├── results-export-json.ts  # JSON issue export formatter
│   │   ├── results-export-ai.ts    # AI JSON export formatter and source attachment
│   │   ├── download-text-file.ts   # Browser/WebView "Save As" helper
│   │   ├── live-edit.ts         # Code viewer edit mode: live lint, autocomplete, save
│   │   ├── path.ts              # Path/project-root resolution helpers (pure functions)
│   │   ├── progress.ts          # Lint results progress bar
│   │   ├── config.ts            # LintConfig: types, load/save, Settings tab formatting UI
│   │   ├── project.ts           # Project dir/compiler/script roots: load/save, Settings UI
│   │   ├── highlight.ts         # Standalone Papyrus syntax highlighter for the
│   │   │                        # code viewer dialog
│   │   ├── main.test.ts         # Vitest unit tests for main.ts and its path/progress/
│   │   │                        # config/project modules
│   │   ├── presets.test.ts      # Vitest unit tests for presets.ts
│   │   ├── code-viewer.test.ts  # Vitest unit tests for code-viewer.ts
│   │   ├── results-list.test.ts # Vitest unit tests for results-list.ts
│   │   ├── results-filter.test.ts # Vitest unit tests for results-filter.ts
│   │   ├── live-edit.test.ts    # Vitest unit tests for live-edit.ts
│   │   ├── highlight.test.ts    # Vitest unit tests for highlight.ts
│   │   ├── test/fixture.ts      # Shared jsdom DOM fixture for the UI tests
│   │   ├── test/mocks.ts        # Shared Tauri spies for the UI tests
│   │   ├── test/harness.ts      # Shared helpers/hooks for the UI tests
│   │   └── styles.css
│   ├── index.html            # Frontend entry point (Vite)
│   ├── package.json          # npm scripts/deps for the frontend and Tauri CLI
│   ├── src-tauri/            # Tauri desktop app shell (Rust)
│   │   └── src/
│   │       ├── main.rs           # Binary entry point: no args -> lib::run() (GUI),
│   │       │                     # args -> papyrus_lint_cli::run() (CLI mode)
│   │       ├── lib.rs            # Façade: registers Tauri commands from the
│   │       │                     # modules below and starts the GUI
│   │       ├── meta.rs           # get_app_version, list_rule_tags
│   │       ├── files.rs          # Achlist/directory listing, .psc read/write/
│   │       │                     # hash/parse, in-memory parse/lint
│   │       ├── lint_config.rs    # papyrus-lint.yaml, compiler path, compile_check,
│   │       │                     # script roots, project info
│   │       ├── config_presets.rs # Built-in and user configuration presets
│   │       ├── lint.rs           # ProjectLintContext, lint_psc_file, compile_psc_file, list_script_members
│   │       └── repair.rs         # Apply/preview fixes and per-line @disable
│   └── crates/
│       ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
│       │   └── src/               # for the Papyrus language. No lint rules live
│       │       ├── lexer.rs        # here — see papyrus-lints below.
│       │       ├── token.rs
│       │       ├── ast.rs
│       │       └── parser.rs
│       ├── papyrus-ast-cache/    # Standalone crate: disk-backed cache of
│       │   └── src/lib.rs         # parsed .psc ASTs/token streams; re-exported
│       │                          # by papyrus-lint-core as its own ast_cache
│       │                          # module
│       ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
│       │   ├── build.rs           # (not the AST) so they still run on scripts
│       │   └── src/                # that don't parse cleanly.
│       │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
│       │       ├── config.rs                  # Config type (YAML-deserializable) passed
│       │       │                              # to every check/fix job
│       │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
│       │       └── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
│       │                                        # via a build-time-generated array
│       ├── papyrus-lint-config/  # Locates/loads/saves a project's
│       │   └── src/               # papyrus-lint.yaml and presets
│       ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
│       │   └── src/               # and the CLI, independent of Tauri:
│       │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
│       │       ├── script_locator.rs   # Finds .psc files by name under
│       │       │                       # scripts/source or source/scripts
│       │       └── function_table/     # Cross-script function signature lookup,
│       │                               # for the argument/return type check lints
│       ├── papyrus-lint-output/  # Plain-text/JSON/AI-export report formatting,
│       │   └── src/               # shared by papyrus-lint-cli and the desktop
│       │                          # app's Tauri commands (app/src-tauri/src/
│       │                          # export.rs), so a diagnostic's exported
│       │                          # shape can't drift between the CLI and GUI
│       └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-psc>`: lints an
│           ├── src/                # achlist's scripts against its project's
│           │   ├── lib.rs           # run() + public API; also linked into
│           │   │                    # src-tauri for its CLI mode
│           │   ├── project.rs       # Project-root discovery from .psc paths
│           │   ├── output/          # CLI-only glue (format selection, --tag/
│           │   │                    # quiet filtering, writing the report) on
│           │   │                    # top of papyrus-lint-output's formatters
│           │   ├── init.rs          # `init` / `preset add`
│           │   ├── blob.rs          # `--blob` in-memory lint
│           │   ├── doctor.rs        # `doctor` subcommand
│           │   ├── test_support.rs  # Shared helpers for each file's unit tests
│           │   └── main.rs          # Thin binary entry point around lib::run()
│           └── tests/               # Binary e2e tests, one file per src module
├── shared/
│   ├── images/               # Images used by README.md (logo, screenshots)
│   └── rules/                 # One <id>.json per lint rule (see "Adding
│                                # lint rules" below); shared/rules.json,
│                                # read by build.rs/pages/build.py, is
│                                # generated from these and git-ignored
├── rules/
│   ├── forbidden-functions.yaml    # Calls discouraged or forbidden by policy
│   ├── slow-functions.yaml         # Slow calls and faster alternatives
│   ├── native-methods.yaml         # Base-game native functions
│   ├── native-types.yaml           # Native engine class hierarchy fallback
│   ├── native-globals.yaml         # Native singleton scripts always called
│   │                                # by literal name
│   ├── actor-values.yaml           # Skyrim's built-in Actor Values
│   ├── known-events.yaml           # Curated native event signatures
│   └── update-event-handlers.yaml  # RegisterFor*/Event pairs; all of these
│                                    # are compiled in by papyrus-lints/build.rs
│                                    # or papyrus-lint-core/build.rs
├── SublimeLinter-contrib-papyrus-lint/  # SublimeLinter integration, commands,
│                                           # and Python unit tests
├── vscode-extension/        # VS Code integration for linting/fixing .psc files
│   ├── src/                 # Extension and diagnostic conversion logic
│   └── test/                # Node-based unit tests
├── pages/                   # Source for the GitHub Pages discoverability
│                              # site (papyrus-lint.idrinth.de), built by
│                              # pages/build.py from README.md and docs/*
├── docs/                    # rules.json (rule metadata), the default config
│   │                          # (papyrus-lint.default.yaml), presets/, JSON
│   │                          # schemas, examples.md, nexuspage.bbcode
│   └── agent/                 # Depth (CI, Pages, releases, implementation
│                                # notes) for AI agents; loaded only when the
│                                # task needs it — see AGENTS.md's routing table
└── docker/                  # Dockerfile/entrypoint for the release CLI image
                               # published to GitHub Container Registry
```

`papyrus-parser`, `papyrus-ast-cache`, `papyrus-lints`, `papyrus-lint-config`,
`papyrus-lint-core`, `papyrus-lint-output`, and `papyrus-lint-cli` are separate
crates (not yet Cargo workspace members, just path dependencies of each other
and of `app/src-tauri`) so the lint engine and project-resolution logic stay
reusable independent of the Tauri app — which is what lets `papyrus-lint-cli`
link against them without pulling in Tauri (and its system GUI dependencies) at all.
`app/src-tauri` depends on `papyrus-lint-cli` too, purely for its `run()` function
(its `main.rs` calls straight into it for CLI mode), not for the `PapyrusLinterCLI`
binary target that crate also defines.

Agent-oriented guidance lives in [`AGENTS.md`](AGENTS.md) (a short index)
and [`docs/agent/`](docs/agent/) (CI, Pages, releases, implementation
notes). `CLAUDE.md` is a pointer to `AGENTS.md`, not a second copy.

## Development setup

- Frontend (`app/`): `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (typecheck + build). `npm run test` runs the frontend's
  Vitest unit tests (`src/**/*.test.ts`); `npm run test:coverage` runs the
  same suite instrumented with `@vitest/coverage-v8`, printing a text
  report and writing HTML/lcov reports to `coverage/`. `npm run lint` runs
  ESLint (flat config in `eslint.config.js`) over `src/`.
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
- AST cache crate only: `cargo test` from `app/crates/papyrus-ast-cache/`.
- Lints crate only: `cargo test` from `app/crates/papyrus-lints/`.
- Config crate only: `cargo test` from `app/crates/papyrus-lint-config/`.
- Shared project-resolution crate only: `cargo test` from
  `app/crates/papyrus-lint-core/`.
- Output formatting crate only: `cargo test` from
  `app/crates/papyrus-lint-output/`.
- CLI: `cargo run --manifest-path app/crates/papyrus-lint-cli/Cargo.toml --
  <path-to-achlist>`, or `cargo build --release --manifest-path
  app/crates/papyrus-lint-cli/Cargo.toml` for a standalone `PapyrusLinterCLI`
  binary. `cargo test` from `app/crates/papyrus-lint-cli/` runs its tests.

The desktop shell is built with [Tauri](https://tauri.app/), so building it
requires Tauri's platform prerequisites (a Rust toolchain, plus the usual
webview dependencies for your OS — see the
[Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)).

## Before opening a pull request

CI (`.github/workflows/ci.yml`) runs on every pull request and on pushes to
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
  `app/crates/papyrus-ast-cache`, `app/crates/papyrus-lints`,
  `app/crates/papyrus-lint-config`, `app/crates/papyrus-lint-core`,
  `app/crates/papyrus-lint-output`, and
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
parse cleanly. Follow that approach for a new rule where practical, then
register it with a `mod` in `app/crates/papyrus-lints/src/lib.rs` and add
`shared/rules/<id>.json` (`repair_order` if `registry::apply_repairs` should
auto-fix it). Every source-level check is
`check(source, ast, tokens, config, external)` and every `apply_repairs`
fix is `repair(source, ast, tokens, config)`. `Rules`,
`default_rules()`, `collect_diagnostics`, `apply_repairs`,
`KNOWN_RULE_IDS`, `FIXABLE_RULE_IDS`, and `RULE_TAGS` are generated by
`build.rs` — do not hand-edit them. Set `"enabled_by_default": false` on
the `shared/rules/<id>.json` entry for opt-in rules.

After adding or editing a `shared/rules/*.json` file, run `python3
.github/scripts/build_rules_json.py` to regenerate the git-ignored
`shared/rules.json` those generated files (and `pages/build.py`) actually
read — do this before building or testing anything below.

A lint/fix job receives a `&papyrus_lints::Config`, deserialized from a
project's optional `papyrus-lint.yaml`/`.yml`, so user-configurable behavior
should be read from there rather than added as a separate parameter. Add tests
for diagnostics, disable comments, configuration, and repairs as applicable,
and update `shared/rules/<id>.json` and the configuration
examples (`configuration/papyrus-lint.default.yaml`, `docs/nexuspage.bbcode`).
`registry.rs`'s `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS`, `tags.rs`'s
`RULE_TAGS`, `config.rs`'s `Rules`, and the check/repair dispatch are all
compiled from the generated `shared/rules.json` by `build.rs`, so they
never need hand-editing.

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

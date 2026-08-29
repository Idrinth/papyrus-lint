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
│   │   ├── main.ts              # Drag-and-drop UI logic, calls into Tauri commands
│   │   ├── highlight.ts         # Standalone Papyrus syntax highlighter for the
│   │   │                        # code viewer dialog
│   │   ├── main.test.ts         # Vitest unit tests for main.ts
│   │   ├── highlight.test.ts    # Vitest unit tests for highlight.ts
│   │   ├── test/fixture.ts      # Shared jsdom DOM fixture for main.test.ts
│   │   └── styles.css
│   ├── index.html            # Frontend entry point (Vite)
│   ├── package.json          # npm scripts/deps for the frontend and Tauri CLI
│   └── src-tauri/            # Tauri desktop app shell (Rust)
│       └── src/
│           ├── main.rs           # Binary entry point: no args -> lib::run() (GUI),
│           │                     # args -> papyrus_lint_cli::run() (CLI mode)
│           ├── lib.rs            # Registers Tauri commands (parse_achlist_file,
│           │                     # parse_papyrus_script, lint_papyrus_script,
│           │                     # parse_psc_file, load_lint_config, lint_psc_file,
│           │                     # repair_psc_file), built on papyrus-lint-core
│           └── compiler.rs        # Runs PapyrusCompiler.exe for the "Compile" button
├── resources/                # Images used by README.md (logo, screenshots)
├── rules/
│   ├── forbidden-functions.yaml  # Calls discouraged or forbidden by policy
│   └── slow-functions.yaml       # Slow calls and faster alternatives; both are
│                                  # compiled in by papyrus-lints/build.rs
├── SublimeLinter-contrib-papyrus-lint/  # SublimeLinter integration, commands,
│                                           # and Python unit tests
├── vscode-extension/        # VS Code integration for linting/fixing .psc files
│   ├── src/                 # Extension and diagnostic conversion logic
│   └── test/                # Node-based unit tests
└── crates/
    ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
    │   └── src/               # for the Papyrus language. No lint rules live
    │       ├── lexer.rs        # here — see papyrus-lints below.
    │       ├── token.rs
    │       ├── ast.rs
    │       └── parser.rs
    ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
    │   ├── build.rs           # (not the AST) so they still run on scripts
    │   └── src/                # that don't parse cleanly.
    │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
    │       ├── config.rs                  # Config type (YAML-deserializable) passed
    │       │                              # to every check/fix job
    │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
    │       └── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
    │                                        # via a build-time-generated array
    ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
    │   └── src/               # and the CLI, independent of Tauri:
    │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
    │       ├── config.rs       # Locates/loads a project's papyrus-lint.yaml
    │       ├── script_locator.rs   # Finds .psc files by name under
    │       │                       # scripts/source or source/scripts
    │       └── function_table.rs   # Cross-script function signature lookup,
    │                               # for the argument/return type check lints
    └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-psc>`: lints an
        └── src/                # achlist's scripts against its project's
            ├── lib.rs           # papyrus-lint.yaml and prints the results.
            │                    # run() here is the shared logic; also
            │                    # linked into app/src-tauri for its CLI mode.
            └── main.rs          # Thin binary entry point around lib::run()
```

`papyrus-parser`, `papyrus-lints`, `papyrus-lint-core`, and
`papyrus-lint-cli` are separate crates (not yet Cargo workspace members,
just path dependencies of each other and of `app/src-tauri`) so the lint
engine and project-resolution logic stay reusable independent of the Tauri
app — which is what lets `papyrus-lint-cli` link against them without
pulling in Tauri (and its system GUI dependencies) at all. `app/src-tauri`
depends on `papyrus-lint-cli` too, purely for its `run()` function (its
`main.rs` calls straight into it for CLI mode), not for the
`PapyrusLinterCLI` binary target that crate also defines.

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
- Parser crate only: `cargo test` from `crates/papyrus-parser/`.
- Lints crate only: `cargo test` from `crates/papyrus-lints/`.
- Shared project-resolution crate only: `cargo test` from
  `crates/papyrus-lint-core/`.
- CLI: `cargo run --manifest-path crates/papyrus-lint-cli/Cargo.toml --
  <path-to-achlist>`, or `cargo build --release --manifest-path
  crates/papyrus-lint-cli/Cargo.toml` for a standalone `PapyrusLinterCLI`
  binary. `cargo test` from `crates/papyrus-lint-cli/` runs its tests.

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
- **Rust test job**: a matrix over `app/src-tauri`, `crates/papyrus-parser`,
  `crates/papyrus-lints`, `crates/papyrus-lint-core`, and
  `crates/papyrus-lint-cli` runs each crate's tests via `cargo llvm-cov`.
  If you touched any of those crates, run `cargo test` (or `cargo
  llvm-cov`, to also see coverage — see the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) docs for
  setup) from that crate's directory to make sure its suite still passes.

Keep pull requests focused on a single change, and use the PR template's
checklist — contributions are permanent and unpaid; only open a PR once
you're comfortable with both.

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

Lint rules live in `crates/papyrus-lints/src`; the complete current set and
each rule's behavior are documented in the [Implemented Lints
table](README.md#implemented-lints). Rules generally inspect raw source or
lexer tokens so they keep running on scripts that do not parse cleanly. Follow
that approach for a new rule where practical, register its check (and optional
repair) in `crates/papyrus-lints/src/lib.rs`, and add its enable switch and
default in `crates/papyrus-lints/src/config.rs`.

A lint/fix job receives a `&papyrus_lints::Config`, deserialized from a
project's optional `papyrus-lint.yaml`/`.yml`, so user-configurable behavior
should be read from there rather than added as a separate parameter. Add tests
for diagnostics, disable comments, configuration, and repairs as applicable,
and update the README's lint table, rule-id list, and configuration example.

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

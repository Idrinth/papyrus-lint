# Papyrus Lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app and a CLI. The desktop app is a Tauri (Rust + TypeScript) app: the
frontend lets a user drop a `.achlist` file, and the Rust backend resolves
the listed files, parses any `.psc` (Papyrus source) files among them, and
lints them. The CLI (`app/crates/papyrus-lint-cli`) does the same thing
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
│   │       │                     # then strips personal data from the compiled .pex
│   │       └── pex_header.rs      # Parses a compiled .pex file's header just far
│   │                             # enough to blank its userName/machineName fields
│   └── crates/
│       ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
│       │   └── src/               # for the Papyrus language. No lint rules live
│       │       ├── lexer.rs        # here — see papyrus-lints below.
│       │       ├── token.rs
│       │       ├── ast.rs
│       │       └── parser.rs
│       ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
│       │   ├── build.rs           # (not the AST) so they still run on scripts
│       │   └── src/                # that don't parse cleanly.
│       │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
│       │       ├── config.rs                  # Config type (YAML-deserializable) passed
│       │       │                              # to every check/fix job
│       │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
│       │       └── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
│       │                                        # via a build-time-generated array
│       ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
│       │   └── src/               # and the CLI, independent of Tauri:
│       │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
│       │       ├── config.rs       # Locates/loads a project's papyrus-lint.yaml
│       │       ├── script_locator.rs   # Finds .psc files by name under
│       │       │                       # scripts/source or source/scripts
│       │       ├── function_table.rs   # Cross-script function signature lookup,
│       │       │                       # for the argument/return type check lints
│       │       └── native_types.rs     # Fallback Extends hierarchy for native
│       │                               # engine types (Actor, ObjectReference,
│       │                               # Form, ...) with no .psc in the project;
│       │                               # reads rules/native-types.yaml via a
│       │                               # build-time-generated array (build.rs)
│       └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-psc>`: lints an
│           └── src/                # achlist's scripts against its project's
│               ├── lib.rs           # papyrus-lint.yaml and prints the results.
│               │                    # run() here is the shared logic; also
│               │                    # linked into src-tauri for its CLI mode.
│               └── main.rs          # Thin binary entry point around lib::run()
├── resources/                # Images used by README.md (logo, screenshots)
├── rules/
│   ├── forbidden-functions.yaml  # Calls discouraged or forbidden by policy
│   ├── slow-functions.yaml       # Slow calls and their faster alternatives; both
│   │                              # files are compiled in by papyrus-lints/build.rs
│   └── native-types.yaml         # Native engine class hierarchy fallback (see
│                                  # papyrus-lint-core/src/native_types.rs above);
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
already exist. A separate `editor-plugins` job runs independently,
packages the VS Code extension into a `.vsix` (via `@vscode/vsce`)
and the `SublimeLinter-contrib-papyrus-lint` directory into a `.zip`, and
attaches both to the same release.

## Merging

Before merging a pull request, make sure its branch is up to date with
`the-one` (the default branch). Merge or rebase `the-one` into the branch
first if it has fallen behind, so CI runs against the current base.

## Current state

The parser (`app/crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence.

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
`papyrus-lint.yml` in the project root. An achlist's parent is the project
root; for a bare `.psc` in a conventional `Scripts/Source` or
`Source/Scripts` tree, the CLI infers the root two directories above it.
Configuration controls formatting, lint enablement, complexity thresholds, CLI failure
levels, and the compiler path. See the [README configuration
reference](README.md#configuration) for the complete schema and defaults.

## Keeping agent instructions synchronized

`AGENTS.md` and `CLAUDE.md` must contain the same project guidance. Whenever
one file is updated, make the equivalent update to the other file in the same
change and verify that the two files remain identical.

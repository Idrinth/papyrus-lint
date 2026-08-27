# Papyrus Lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app. The UI is a Tauri (Rust + TypeScript) app: the frontend lets a user
drop a `.achlist` file, and the Rust backend resolves the listed files,
parses any `.psc` (Papyrus source) files among them, and lints them.

## Project structure

```
.
├── src/                    # Frontend (TypeScript, vanilla, no framework)
│   ├── main.ts              # Drag-and-drop UI logic, calls into Tauri commands
│   ├── highlight.ts         # Standalone Papyrus syntax highlighter for the
│   │                        # code viewer dialog
│   ├── main.test.ts         # Vitest unit tests for main.ts
│   ├── highlight.test.ts    # Vitest unit tests for highlight.ts
│   ├── test/fixture.ts      # Shared jsdom DOM fixture for main.test.ts
│   └── styles.css
├── index.html               # Frontend entry point (Vite)
├── src-tauri/               # Tauri desktop app shell (Rust)
│   └── src/
│       ├── main.rs           # Binary entry point, delegates to lib::run()
│       ├── lib.rs            # Registers Tauri commands (parse_achlist_file,
│       │                     # parse_papyrus_script, lint_papyrus_script,
│       │                     # parse_psc_file, load_lint_config, lint_psc_file,
│       │                     # repair_psc_file)
│       ├── achlist.rs        # Parses .achlist files (JSON arrays of paths)
│       ├── config.rs          # Locates/loads a project's papyrus-lint.yaml
│       └── script_locator.rs  # Finds .psc files by name under scripts/source
│                               # or source/scripts
├── rules/
│   └── forbidden-functions.yaml  # Data for the "forbidden function usage" lint;
│                                  # compiled into Rust by papyrus-lints' build.rs
└── crates/
    ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
    │   └── src/               # for the Papyrus language. No lint rules live
    │       ├── lexer.rs        # here — see papyrus-lints below.
    │       ├── token.rs
    │       ├── ast.rs
    │       └── parser.rs
    └── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
        ├── build.rs           # (not the AST) so they still run on scripts
        └── src/                # that don't parse cleanly.
            ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
            ├── config.rs                  # Config type (YAML-deserializable) passed
            │                              # to every check/fix job
            ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
            └── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
                                             # via a build-time-generated array
```

`papyrus-parser` and `papyrus-lints` are separate crates (not yet Cargo
workspace members, just path dependencies of `src-tauri` and, for
`papyrus-lints`, of `papyrus-parser`'s lexer) so they stay reusable
independent of the Tauri app — e.g. for a future CLI or test harness.

## Development

- Frontend: `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (typecheck + build). `npm run test` runs the frontend's
  Vitest unit tests (`src/**/*.test.ts`); `npm run test:coverage` runs the
  same suite instrumented with `@vitest/coverage-v8`, printing a text
  report and writing HTML/lcov reports to `coverage/`. `npm run lint` runs
  ESLint (flat config in `eslint.config.js`) over `src/`, using
  `typescript-eslint`'s recommended rules plus `@vitest/eslint-plugin`'s
  recommended rules on test files.
  - `typescript-eslint` doesn't yet support TypeScript 7 (this repo's
    `typescript` devDependency), so `package.json` installs it under an
    npm alias: `typescript` resolves to the `@typescript/typescript6` shim
    (TS 6, satisfying typescript-eslint) and the real TS 7 compiler is
    installed separately as `@typescript/native`, which is what `tsc`
    (used by `npm run build`) actually runs. See
    https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0.
- Full desktop app: `npm run tauri dev` / `npm run tauri build`.
- Rust backend only: `cargo check` / `cargo test` from `src-tauri/`.
- Parser crate only: `cargo test` from `crates/papyrus-parser/`.
- Lints crate only: `cargo test` from `crates/papyrus-lints/`.
- Rust coverage for any of the three crates above: `cargo llvm-cov
  --manifest-path <crate>/Cargo.toml` (requires the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) subcommand
  and the `llvm-tools-preview` rustup component).

## CI (`.github/workflows/ci.yml`)

- **Frontend job**: `npm ci`, then `npm run lint` (ESLint), `npm run
  test:coverage` (Vitest unit tests, instrumented for coverage), and `npm
  run build` (typecheck & Vite build). The text coverage summary is
  posted to the job's step summary and the full HTML/lcov report is
  uploaded as the `frontend-coverage` artifact.
- **Rust build job**: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  and `cargo check`, all run against `src-tauri/Cargo.toml`.
- **Rust test job**: a matrix over `src-tauri`, `crates/papyrus-parser`,
  and `crates/papyrus-lints` runs each crate's tests via `cargo llvm-cov`.
  Each matrix leg posts its text coverage summary to the job's step
  summary and uploads its lcov report as a `rust-coverage-<crate>`
  artifact.

Note: CI runs on pushes to `the-one` (the default branch, not `main`) and
on all pull requests.

## Releases (`.github/workflows/release.yml`)

Pushing a tag matching `v*.*.*` triggers a release job that builds the
Tauri desktop app on Linux, macOS, and Windows (via
`tauri-apps/tauri-action`) and attaches each platform's build output
(installers/bundles) to a GitHub release for that tag, creating the
release if it doesn't already exist.

## Merging

Before merging a pull request, make sure its branch is up to date with
`the-one` (the default branch). Merge or rebase `the-one` into the branch
first if it has fallen behind, so CI runs against the current base.

## Current state

The parser (`crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence.

Two lints are implemented in `crates/papyrus-lints`:

- **Trailing whitespace** (`trailing_whitespace.rs`): flags lines ending in
  spaces or tabs, and can also repair them via its `repair()` function,
  which strips the trailing spaces/tabs from each line while preserving
  line endings (`\n`/`\r\n`) and a missing final newline.
- **Forbidden/discouraged function usage** (`forbidden_functions.rs`):
  flags calls to functions listed in `rules/forbidden-functions.yaml`.
  That YAML is compiled into a static Rust array by
  `crates/papyrus-lints/build.rs` at build time, so linting never parses
  YAML at runtime.

Both lints work on lexer tokens/raw text rather than the parsed AST, so
they still run on scripts that don't parse cleanly. They're exposed to the
frontend via the `lint_papyrus_script` and `lint_psc_file` Tauri commands.
`papyrus_lints::repair()` aggregates every available automatic fix
(currently just trailing whitespace) and is exposed via the
`repair_psc_file` Tauri command, which rewrites the `.psc` file on disk and
returns the diagnostics that remain. See README.md for the remaining
planned lints and fixes.

`lint()` and `repair()` both take a `&papyrus_lints::Config` — deserialized
from a project's optional `papyrus-lint.yaml`/`.yml` file (default:
`semicolon: false`, `indentation: tab`, `indentation_width: 4`, and a
`rules` map with every ruleset enabled) — so it's available to every
check/fix job, and it now drives the semicolon and indentation fixers
directly (via `Config::semicolon_style()`/`Config::indentation_unit()`)
rather than those being passed separately. `Config::rules` (see
`config.rs`) lets a project disable any individual ruleset by name; both
`lint_with_external_arguments()` and `repair()` skip a disabled ruleset's
check/fix.
`src-tauri/src/config.rs` locates that file next to the dropped
`.achlist` file and exposes `load_lint_config`/`save_lint_config` Tauri
commands. The frontend treats the config file as the source of truth for
its formatting controls (trailing semicolons, indentation style/width):
it loads the config for the most recently opened project on startup
(remembered via `localStorage`) and after every achlist drop, applies it
to those controls, and writes any change made to them straight back to
the file via `save_lint_config`.

# Contributing to Papyrus Lint

Thanks for your interest in contributing! This document covers how the
project is laid out, how to set up a development environment, and what's
expected of a pull request.

## Project structure

```
.
├── src/                    # Frontend (TypeScript, vanilla, no framework)
│   ├── main.ts              # Drag-and-drop UI logic, calls into Tauri commands
│   └── styles.css
├── index.html               # Frontend entry point (Vite)
├── src-tauri/               # Tauri desktop app shell (Rust)
│   └── src/
│       ├── main.rs           # Binary entry point, delegates to lib::run()
│       ├── lib.rs            # Registers Tauri commands (parse_achlist_file,
│       │                     # parse_papyrus_script, parse_psc_file)
│       ├── achlist.rs        # Parses .achlist files (JSON arrays of paths)
│       └── script_locator.rs  # Finds .psc files by name under scripts/source
│                               # or source/scripts
└── crates/
    └── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
        └── src/               # for the Papyrus language. No lint rules live
            ├── lexer.rs        # here yet — this is the parsing foundation
            ├── token.rs        # the lints described in README.md will be
            ├── ast.rs          # built on top of.
            └── parser.rs
```

`papyrus-parser` is a separate crate (not yet a Cargo workspace member,
just a path dependency of `src-tauri`) so the parsing logic stays reusable
independent of the Tauri app — e.g. for a future CLI or test harness.

## Development setup

- Frontend: `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (typecheck + build).
- Full desktop app: `npm run tauri dev` / `npm run tauri build`.
- Rust backend only: `cargo check` / `cargo test` from `src-tauri/`.
- Parser crate only: `cargo test` from `crates/papyrus-parser/`.

The desktop shell is built with [Tauri](https://tauri.app/), so building it
requires Tauri's platform prerequisites (a Rust toolchain, plus the usual
webview dependencies for your OS — see the
[Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)).

## Before opening a pull request

CI (`.github/workflows/ci.yml`) runs on every pull request and on pushes to
`the-one` (the default branch — not `main`). Make sure your change passes
the same checks locally first:

- **Frontend**: `npm ci` then `npm run build` (typecheck + Vite build).
- **Rust**, from `src-tauri/`:
  - `cargo fmt -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo check`

If you touched `crates/papyrus-parser`, also run `cargo test` from that
directory to make sure the lexer/parser test suite still passes.

Keep pull requests focused on a single change, and use the PR template's
checklist — contributions are permanent and unpaid; only open a PR once
you're comfortable with both.

## Code style

- Rust code is formatted with `cargo fmt` and linted with `clippy`
  (warnings are treated as errors in CI). Run both before committing.
- TypeScript is checked with `tsc` as part of `npm run build`; there is no
  separate linter configured yet.
- Match the existing style of the file you're editing (naming, module
  layout, etc.) rather than introducing a new convention.

## Adding lint rules

No lint rules are implemented yet — `papyrus-parser` currently only
provides the lexer, AST, and parser that lint rules will be built on top
of. See the "Planned Lints" section of [README.md](README.md) for the list
of lints this project intends to implement. If you want to work on one,
open an issue or comment on an existing one first to avoid duplicate work,
since the rule infrastructure is still taking shape.

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

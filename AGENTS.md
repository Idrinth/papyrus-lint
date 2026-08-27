# papyrus-lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app. The UI is a Tauri (Rust + TypeScript) app: the frontend lets a user
drop a `.achlist` file, and the Rust backend resolves the listed files,
parses any `.psc` (Papyrus source) files among them, and lints them.

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
│       │                     # parse_papyrus_script, lint_papyrus_script,
│       │                     # parse_psc_file, lint_psc_file)
│       ├── achlist.rs        # Parses .achlist files (JSON arrays of paths)
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
            ├── lib.rs                     # Diagnostic type + lint() entry point
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
  `npm run build` (typecheck + build).
- Full desktop app: `npm run tauri dev` / `npm run tauri build`.
- Rust backend only: `cargo check` / `cargo test` from `src-tauri/`.
- Parser crate only: `cargo test` from `crates/papyrus-parser/`.
- Lints crate only: `cargo test` from `crates/papyrus-lints/`.

## CI (`.github/workflows/ci.yml`)

- **Frontend job**: `npm ci` then `npm run build` (typecheck & Vite build).
- **Rust job**: `cargo fmt --check`, `cargo clippy -- -D warnings`, and
  `cargo check`, all run against `src-tauri/Cargo.toml`.

Note: CI runs on pushes to `the-one` (the default branch, not `main`) and
on all pull requests.

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
  spaces or tabs.
- **Forbidden/discouraged function usage** (`forbidden_functions.rs`):
  flags calls to functions listed in `rules/forbidden-functions.yaml`.
  That YAML is compiled into a static Rust array by
  `crates/papyrus-lints/build.rs` at build time, so linting never parses
  YAML at runtime.

Both work on lexer tokens/raw text rather than the parsed AST, so they
still run on scripts that don't parse cleanly. They're exposed to the
frontend via the `lint_papyrus_script` and `lint_psc_file` Tauri commands.
See README.md for the remaining planned lints.

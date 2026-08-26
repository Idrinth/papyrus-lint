# papyrus-lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app. The UI is a Tauri (Rust + TypeScript) app: the frontend lets a user
drop a `.archlist` file, and the Rust backend resolves the listed files and
parses any `.psc` (Papyrus source) files among them.

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
│       ├── lib.rs            # Registers Tauri commands (parse_archlist_file,
│       │                     # parse_papyrus_script, parse_psc_file)
│       ├── archlist.rs        # Parses .archlist files (JSON arrays of paths)
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

## Development

- Frontend: `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (typecheck + build).
- Full desktop app: `npm run tauri dev` / `npm run tauri build`.
- Rust backend only: `cargo check` / `cargo test` from `src-tauri/`.
- Parser crate only: `cargo test` from `crates/papyrus-parser/`.

## CI (`.github/workflows/ci.yml`)

- **Frontend job**: `npm ci` then `npm run build` (typecheck + Vite build).
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
standard precedence. No lint rules are implemented yet — see README.md for
the planned lint list.

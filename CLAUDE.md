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
├── rules/
│   └── forbidden-functions.yaml  # Data for the "forbidden function usage" lint;
│                                  # compiled into Rust by papyrus-parser's build.rs
└── crates/
    └── papyrus-parser/       # Standalone Rust crate: lexer, AST, parser, and
        ├── build.rs           # lints for the Papyrus language.
        └── src/
            ├── lexer.rs
            ├── token.rs
            ├── ast.rs
            ├── parser.rs
            └── lints/
                └── forbidden_functions.rs  # Reads rules/forbidden-functions.yaml
                                              # via a build-time-generated array
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

## Current state

The parser (`crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence.

One lint is implemented: "forbidden/discouraged function usage"
(`crates/papyrus-parser/src/lints/forbidden_functions.rs`), driven by
`rules/forbidden-functions.yaml`. `crates/papyrus-parser/build.rs` compiles
that YAML into a static Rust array at build time, so linting never parses
YAML at runtime. It's exposed to the frontend via the `lint_papyrus_script`
and `lint_psc_file` Tauri commands. See README.md for the remaining planned
lints.

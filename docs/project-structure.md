# Project structure

This is the canonical overview of the repository layout, shared by contributors
and coding agents. Update it when responsibilities move; do not copy the tree
into `CONTRIBUTING.md` or `AGENTS.md`.

The tree below intentionally stops at architectural boundaries. It lists the
directories and entry points a contributor needs to find a feature without
trying to inventory every lint rule, test, generated file, or image.

## Top-level layout

```text
.
├── app/                              # Desktop app, frontend, and Rust crates
├── configuration/                    # Shipped default config and built-in presets
├── docs/                             # User, contributor, and agent documentation
├── docker/                           # Release container image
├── fixtures/                         # Generated lint-output baselines for supported games
├── pages/                            # GitHub Pages source and build/test tooling
├── schema/                           # JSON schemas for config and exported reports
├── shared/                           # Cross-component metadata, assets, and base scripts
├── SublimeLinter-contrib-papyrus-lint/ # SublimeLinter package
├── templates/                        # Release-time Nexus page template
├── vscode-extension/                 # VS Code extension
└── .github/                          # CI, release workflows, and repository scripts
```

## Desktop app and frontend

`app/` is an npm/Vite project whose browser-facing code talks to the Rust
backend through Tauri commands.

```text
app/
├── index.html                         # Vite entry document and application dialogs
├── src/                               # Framework-free TypeScript frontend
│   ├── main.ts                        # Composition root and application startup
│   ├── backend.ts                     # Typed Tauri command facade
│   ├── backend-context.ts             # Injectable backend used by UI modules/tests
│   ├── drop.ts / watch.ts             # Project selection and filesystem watching
│   ├── config*.ts / project-*.ts      # Configuration and project settings
│   ├── presets*.ts                    # Preset picker and preset management
│   ├── results-list*.ts               # Finding state, rendering, fixes, and exports
│   ├── results-filter.ts              # Finding filters
│   ├── code-viewer*.ts                # Viewer dialog, actions, compile, and diffs
│   ├── live-edit*.ts                  # Editor linting, highlighting, completion, save
│   ├── autocomplete*.ts               # Papyrus completion helpers
│   ├── highlight.ts / papyrus-source.ts # Source tokenization and presentation
│   ├── test/                           # Shared Vitest DOM fixture, mocks, and harness
│   └── styles/                         # Feature-level stylesheets
├── scripts/                           # Config type generation and link injection
├── e2e/                               # Playwright layout/browser tests
├── public/                            # Static Vite assets
└── src-tauri/                         # Rust/Tauri application package
    ├── src/main.rs                    # GUI/CLI process entry point
    ├── src/lib.rs                     # Tauri builder and command registration
    ├── src/files.rs                   # Script/project discovery and source I/O commands
    ├── src/lint.rs                    # Desktop lint orchestration
    ├── src/repair.rs                  # Fix and suppression commands
    ├── src/lint_config.rs             # Configuration-facing Tauri commands
    ├── src/config_presets.rs          # Preset-facing Tauri commands
    ├── src/export.rs                  # Text, JSON, and AI report commands
    ├── src/project_root.rs            # Project-root command adapter
    ├── src/meta.rs                    # Version and rule metadata commands
    ├── capabilities/                  # Tauri v2 capability declarations
    └── tests/                         # Binary-level integration tests
```

Frontend tests normally sit beside their module as `*.test.ts`. Rust unit tests
similarly use sibling `*_tests.rs` files (and, where useful, a directory split
by scenario) so implementation files do not become test containers.

## Reusable Rust crates

The eleven crates under `app/crates/` are independent path dependencies, **not a
Cargo workspace**. Run Cargo commands against each crate's own `Cargo.toml`.
Each crate's README holds its local invariants and test command; do not
copy those into `docs/agent/`.

| Crate | Responsibility |
| --- | --- |
| `papyrus-lint-globals` | Dependency-light shared types and constants, including the supported-game model. |
| `papyrus-parser` | Lexer, parser, AST, visitors, comment annotations, and type inference for Papyrus. |
| `papyrus-ast-cache` | Disk and bundled-base-script cache for parsed ASTs and token streams. |
| `papyrus-collision-cache` | Persistent content-hash cache used to detect conflicting script copies. |
| `papyrus-lints` | Diagnostics, suppression/repair infrastructure, lint visitors, and individual rules. |
| `papyrus-lint-config` | Config discovery and YAML I/O, compiler/game detection, script roots, and presets. |
| `papyrus-lint-core` | Tauri-independent project resolution, cross-script lookup, compilation, and shared workflows. |
| `papyrus-lint-output` | Plain-text, JSON, and AI-report models and formatting shared by GUI and CLI. |
| `papyrus-lint-live` | In-memory source linting shared by CLI `--blob`, the language server, and desktop live edit. |
| `papyrus-lint-cli` | `PapyrusLinterCLI` argument parsing and lint/fix/init/doctor/blob orchestration. |
| `papyrus-lint-lsp` | Stdio language server (`PapyrusLinterLsp`). Publishes diagnostics, per-issue quick fixes, and `papyrusLint.fixFile` (every automatic fix, via `workspace/applyEdit`). |

Important internal boundaries:

- `papyrus-parser/src/parser/` splits declaration, statement, and expression
  parsing; `visit.rs` defines AST traversal.
- `papyrus-ast-cache/src/ops/` contains load, store, and priming operations.
  `build.rs` packages the archives in `shared/scripts/` for first-run lookup.
- `papyrus-lints/build.rs` and `build_support/` generate rule modules,
  configuration fields, registry dispatch, tags, and embedded lookup tables
  from the generated `shared/rules.json`. Individual rule implementations and
  their sibling tests live in `papyrus-lints/src/`.
- `papyrus-lint-config` separates lint settings, compiler settings, script
  roots, achlist settings, preset files, and YAML merging into focused modules.
- `papyrus-lint-core/src/function_table/` owns lazy cross-script signature and
  ancestry lookup. Other modules cover `.achlist`/`.ppj` input, source encoding,
  project roots, compilation, diffs, parallel work, PEX headers, and stale
  compiled output.
- `papyrus-lint-live` is the in-memory lint pass (`lint_source`) plus config
  resolution for a buffer (`config_from_override`, `config_from_script_path`).
  CLI `--blob`, the LSP document snapshot, and the desktop live-edit Tauri
  command all call it; report formatting, protocol mapping, and the GUI
  highlight layer stay in those crates.
- `papyrus-lint-lsp` is a standalone stdio process. Document diagnostics and
  the code-action re-lint go through `papyrus-lint-live`; protocol framing,
  document sync, and workspace edits stay in this crate.
- `papyrus-lint-cli/src/args/`, `doctor/`, and `output/` contain their respective
  command subsystems. `run_scan.rs`, `run_lint.rs`, `run_fix.rs`, and
  `run_lint_command.rs` form the normal lint/fix pipeline; `src/main.rs` is only
  the binary adapter around the library API.

The Tauri package depends on the reusable crates and invokes
`papyrus_lint_cli::run()` when launched with CLI arguments. This preserves one
CLI implementation while keeping Tauri and its GUI system dependencies out of
the standalone CLI crate. Both surfaces use `papyrus-lint-output`, preventing
their exported report formats from drifting.

## Shared inputs and generated data

```text
configuration/
├── papyrus-lint.default.yaml          # Documented full configuration
└── presets/                          # standard, careful, and strict configs

shared/
├── rules/<id>.json                   # Canonical metadata for one lint rule
├── rules/data/<game>/*.yaml          # Game-specific function/value lookup tables
├── scripts/*.zip                     # Base and extender scripts embedded by caches
├── images/                           # README/site screenshots and branding
├── links.yaml                        # Canonical contact/download/documentation links
└── theme.css                         # Palette shared by the app and website

schema/
├── papyrus-lint.schema.json          # Project configuration schema
├── papyrus-lint-rule.schema.json     # Rule metadata schema
├── papyrus-lint-report.schema.json   # CLI JSON report schema
└── papyrus-lint-ai-export.v*.schema.json # Versioned AI export schemas
```

`shared/rules.json` is a git-ignored build input assembled from
`shared/rules/<id>.json` by `.github/scripts/build_rules_json.py`; never edit or
commit it. Rust build scripts, frontend config generation, Pages, and release
tooling consume that combined file. See `AGENTS.md` and `CONTRIBUTING.md` before
changing rule metadata.

## Editor integrations

`vscode-extension/` is a standalone TypeScript package. `src/extension.ts`
wires together CLI discovery/download (`cli*.ts`), process execution
(`linter.ts`), diagnostics, live linting, initialization, suppressions, code
actions, and `@nodiscard` actions. Node-based module tests live in `test/`.

`SublimeLinter-contrib-papyrus-lint/` is a standalone Python package. Its
`linter.py` adapts CLI output to SublimeLinter; the command modules provide
initialization and fix actions; `cli_download.py` and `cli_hashes.py` manage the
released executable. Tests live in its `tests/` directory.

Both integrations invoke `PapyrusLinterCLI`; neither embeds the Rust lint
engine.

## Website, release, and automation

- `pages/` contains HTML templates, shared includes, CSS/JavaScript, local
  fonts, and small Python build modules. `build.py` renders the main site, every
  configured `docs/*.md` page, rule reference, videos, action documentation,
  coverage, sitemap, and static assets into the git-ignored `pages/dist/`.
  Its `test_*.py` suite exercises build components; `browser_check.py` performs
  Playwright checks against the assembled site.
- `docs/` contains user documentation. `docs/agent/` holds focused maintenance
  notes loaded on demand; this file is the repository-layout reference.
- `.github/workflows/` owns CI, Pages, fixture generation, CodeQL, and releases.
  `.github/scripts/` contains their Python helpers and tests, including rule
  aggregation, Nexus rendering, source metrics, link checks, and release hash
  generation.
- `templates/nexuspage.bbcode` is a release-time source template. Generated
  lint tables and links are inserted during release rather than checked in.
- `docker/` builds the published CLI image and provides its entrypoint.
- `fixtures/` stores expected reports for game/base-script and preset
  combinations; automation regenerates them.

## Where to make a change

| Change | Primary location |
| --- | --- |
| Papyrus syntax or AST behavior | `app/crates/papyrus-parser/` |
| A lint or automatic repair | `app/crates/papyrus-lints/` plus `shared/rules/<id>.json` |
| Config discovery, persistence, or presets | `app/crates/papyrus-lint-config/` |
| Cross-script/project/compiler behavior | `app/crates/papyrus-lint-core/` |
| In-memory / blob lint | `app/crates/papyrus-lint-live/` |
| CLI command behavior | `app/crates/papyrus-lint-cli/` |
| LSP editor adapter | `app/crates/papyrus-lint-lsp/` |
| Shared export shape/formatting | `app/crates/papyrus-lint-output/` |
| Desktop-only backend command | `app/src-tauri/` |
| Desktop UI behavior | `app/src/` |
| VS Code or Sublime integration | `vscode-extension/` or `SublimeLinter-contrib-papyrus-lint/` |
| Website generation | `pages/` |
| CI/release automation | `.github/workflows/` and `.github/scripts/` |

For the desktop UI, see `app/src/README.md`.
For the Tauri shell, editor plugins, and coverage, see `docs/agent/development.md`.
For a crate's own commands, see that crate's `README.md`.

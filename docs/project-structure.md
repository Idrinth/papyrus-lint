# Project structure

This is the single source of truth for the repository layout, shared by
contributors and coding agents. Update this document when the layout changes;
do not copy the tree into `CONTRIBUTING.md` or `AGENTS.md`.


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
│   │   ├── results-export-text.ts  # Plain-text issue export: calls the
│   │   │                           # format_issues_as_text Tauri command
│   │   │                           # (app/src-tauri/src/export.rs)
│   │   ├── results-export-json.ts  # JSON issue export: calls format_issues_as_json;
│   │   │                           # also sortedByPosition, shared with ai.ts
│   │   ├── results-export-ai.ts    # AI JSON export: calls format_issues_for_ai_base for
│   │   │                           # the shared header/configuration/findings/rule_details,
│   │   │                           # then layers on the GUI-only filters/external/repair
│   │   │                           # fields and source attachment
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
│   │   ├── styles.css           # Stylesheet entry point; imports shared/theme.css
│   │   │                        # and the feature stylesheets below
│   │   └── styles/              # App chrome grouped by frontend feature:
│   │       ├── base.css             # Shell, tabs, progress, and contact list
│   │       ├── settings.css         # Compiler, script-root, and lint settings
│   │       ├── results.css          # Drop zone and lint result presentation
│   │       ├── code-viewer.css      # Viewer, editor, and diff presentation
│   │       ├── presets.css          # Config picker and preset management
│   │       └── syntax-highlighting.css # Papyrus token colors
│   ├── e2e/                  # Playwright specs (real Chromium, not jsdom):
│   │   └── layout.spec.ts       # catches element-size/layout regressions
│   ├── playwright.config.ts  # Config for the e2e/ specs above
│   ├── index.html            # Frontend entry point (Vite)
│   ├── package.json          # npm scripts/deps for the frontend and Tauri CLI
│   ├── src-tauri/            # Tauri desktop app shell (Rust)
│   │   └── src/
│   │       ├── main.rs           # Binary entry point: no args -> lib::run() (GUI),
│   │       │                     # args -> papyrus_lint_cli::run() (CLI mode)
│   │       ├── lib.rs            # Façade: registers Tauri commands from the
│   │       │                     # modules below and starts the GUI
│   │       ├── meta.rs           # get_app_version, list_rule_tags
│   │       ├── files.rs          # Achlist/ppj/directory listing, .psc read/write/
│   │       │                     # hash/parse, in-memory parse/lint
│   │       ├── lint_config.rs    # papyrus-lint.yaml, compiler path, compile_check,
│   │       │                     # script roots, project info
│   │       ├── config_presets.rs # Built-in and user configuration presets
│   │       ├── lint.rs           # ProjectLintContext, lint_psc_file, compile_psc_file, list_script_members
│   │       ├── lint_tests.rs     # lint.rs's unit tests, `#[path]`-included as its
│   │       │                     # `mod tests` so lint.rs's own size tracks its
│   │       │                     # actual (small) implementation
│   │       ├── repair.rs         # Apply/preview fixes and per-line @disable
│   │       └── export.rs         # format_issues_as_text/format_issues_as_json/
│   │                             # format_issues_for_ai_base: the "Export
│   │                             # issues"/"Export for AI" buttons' own
│   │                             # formatting, built on papyrus-lint-output
│   └── crates/
│       ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
│       │   └── src/               # for the Papyrus language. No lint rules live
│       │       ├── lexer.rs        # here — see papyrus-lints below.
│       │       ├── token.rs
│       │       ├── ast.rs
│       │       ├── parser.rs
│       │       ├── types.rs        # TypeEnv: variable type tracking for a
│       │       │                   # parsed script, used by lints that need a
│       │       │                   # value's declared type (implicit
│       │       │                   # conversions, non-Bool conditions, ...)
│       │       └── cache.rs        # In-memory memoization of parse()/tokenize()
│       │                           # against the most recently seen source, so
│       │                           # one lint pass over a script only lexes/
│       │                           # parses it once no matter how many lint
│       │                           # rules each ask for their own tokens/AST
│       ├── papyrus-ast-cache/    # Standalone crate: disk-backed cache of
│       │   ├── build.rs           # parsed .psc ASTs/token streams, keyed by
│       │   └── src/               # content MD5 + mtime + linter version;
│       │       ├── lib.rs           # depends only on papyrus-parser, so it's
│       │       │                    # reusable on its own. papyrus-lint-core
│       │       │                    # re-exports it as its own ast_cache
│       │       │                    # module (see below); this crate's own
│       │       │                    # public get/put/ensure_primed API.
│       │       │                    # build.rs compiles shared/skyrim-scripts.zip
│       │       │                    # into a content-addressed AST/token blob
│       │       │                    # that get/get_tokens/ensure_primed consult
│       │       │                    # before the on-disk cache, so vanilla
│       │       │                    # base types hit on first analysis, and
│       │       │                    # that FunctionTable consults by ScriptName
│       │       │                    # when no matching .psc is on disk
│       │       ├── bundled.rs       # Runtime lookup into that blob (MD5 of
│       │       │                    # decoded source → AST/tokens, and
│       │       │                    # lowercased ScriptName → AST; no disk lock)
│       │       ├── bundled_blob.rs  # Binary layout of the blob; shared with
│       │       │                    # build.rs via a #[path] include
│       │       ├── psc_decode.rs    # UTF-8 / Windows-1252 decode used by
│       │       │                    # build.rs (same policy as source_encoding)
│       │       ├── entry.rs         # On-disk entry representation: where a
│       │       │                    # cache entry lives, how it's addressed,
│       │       │                    # and its raw read/write
│       │       ├── ops/              # get/put/ensure_primed semantics built on
│       │       │                     # entry.rs's on-disk primitives, split by
│       │       │                     # responsibility: mod.rs is a facade over
│       │       │                     # load.rs (get/get_tokens), store.rs
│       │       │                     # (put/put_tokens) and prime.rs
│       │       │                     # (ensure_primed, built on both)
│       │       └── version.rs       # MIN_COMPATIBLE_VERSION and the
│       │                            # entry-vs-running-binary compatibility
│       │                            # check
│       ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
│       │   ├── build.rs           # (not the AST) so they still run on scripts
│       │   └── src/                # that don't parse cleanly. Every file with
│       │       │                   # unit tests keeps them in a sibling
│       │       │                   # `<name>_tests.rs`, `#[path]`-included as
│       │       │                   # its `mod tests`, so a file's own size
│       │       │                   # tracks its implementation, not its tests.
│       │       │                   # lib.rs is the exception: its tests live
│       │       │                   # in lib_tests/, split by public entry
│       │       │                   # point the same way papyrus-lint-cli's
│       │       │                   # run_tests/ is split for run().
│       │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
│       │       ├── lib_tests/                 # lib.rs's unit tests, split by the public entry points they cover
│       │       │   ├── mod.rs                   # test module declarations
│       │       │   ├── diagnostic.rs            # Diagnostic::level
│       │       │   ├── lint.rs                  # lint() / extra_diagnostics / @disable
│       │       │   ├── repair.rs                # repair() / repair_filtered / by-tag
│       │       │   ├── repair_external.rs       # ExternalSignatures-aware repair
│       │       │   ├── restrict_to_line.rs      # restrict_to_line / repaired_line
│       │       │   ├── disable.rs               # add_disable_comment / is_disabled
│       │       │   ├── rule_flags.rs            # each config.rules flag via lint()
│       │       │   ├── rule_flags_external.rs   # flags that need ExternalSignatures
│       │       │   └── support.rs               # config_with + fake ExternalSignatures
│       │       ├── config.rs                  # Config type (YAML-deserializable) passed
│       │       │                              # to every check/fix job; YAML file I/O
│       │       │                              # lives in papyrus-lint-config
│       │       ├── config_tests.rs            # config.rs's unit tests
│       │       ├── const_eval.rs              # Shared compile-time folding of
│       │       │                              # literal expressions (`eval_const`,
│       │       │                              # `eval_const_int`); used by the
│       │       │                              # numeric AST lints and static-condition
│       │       ├── const_eval_tests.rs        # const_eval.rs's unit tests
│       │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
│       │       ├── trailing_whitespace_tests.rs # trailing_whitespace.rs's unit tests
│       │       ├── forbidden_functions.rs     # Reads shared/rules/data/forbidden-functions.yaml
│       │       │                              # via a build-time-generated array
│       │       ├── native_function_usage.rs   # Reads shared/rules/data/native-methods.yaml via a
│       │       │                              # build-time-generated array; disabled by
│       │       │                              # default
│       │       └── actor_value.rs             # Flags a call to an Actor Value function
│       │                                      # (GetActorValue, SetActorValue, ...) whose
│       │                                      # argument isn't a known Actor Value; reads
│       │                                      # shared/rules/data/actor-values.yaml via a build-time-
│       │                                      # generated array; disabled by default
│       ├── papyrus-lint-config/  # Locates/loads/saves a project's
│       │   └── src/               # papyrus-lint.yaml (lint settings, compiler
│       │       ├── lib.rs          # path, script roots); depends only on
│       │       │                  # papyrus-lints. lib.rs is a thin facade:
│       │       │                  # module split below, re-exported at the
│       │       │                  # crate root
│       │       ├── project_file.rs # ProjectFile model, YAML load/save/
│       │       │                  # discovery, and the per-field loaders/
│       │       │                  # savers (compiler_path, compile_check,
│       │       │                  # script_roots, lookup_script_roots,
│       │       │                  # strict_achlist_scope) built on it
│       │       ├── comments.rs     # README-synced explanatory comments
│       │       │                  # injected above each saved top-level key
│       │       ├── skyrim.rs       # Windows registry detection of a Skyrim
│       │       │                  # Special Edition install and its vanilla
│       │       │                  # script directories
│       │       ├── compiler.rs     # PapyrusCompiler.exe auto-detection/
│       │       │                  # resolution
│       │       └── presets.rs      # Preset (built-in + user), user-preset
│       │                          # add/save/rename/delete/export, and the
│       │                          # executable-adjacent base-config layering
│       │                          # init/apply use (see Configuration below)
│       ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
│       │   └── src/               # and the CLI, independent of Tauri:
│       │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
│       │       ├── ppj.rs          # Parses .ppj (Papyrus Project XML) files:
│       │       │                   # <Import>/<Folder>/<Script> into search
│       │       │                   # roots and the scripts they name, so a
│       │       │                   # .ppj can drive lint/fix/doctor/init the
│       │       │                   # same way an .achlist does
│       │       ├── lib.rs          # Re-exports papyrus-ast-cache (see above) as
│       │       │                   # this crate's own ast_cache module
│       │       ├── content_hash.rs # MD5 hashing helper for the "Export for AI"
│       │       │                   # redacted-source option / --hash-source
│       │       ├── diff.rs         # unified_diff: renders fix --dry-run's/
│       │       │                   # "Preview fixes"'s standard unified diff
│       │       ├── project_root.rs # Discovers a project's root from a .psc
│       │       │                   # file's position on disk, for the CLI and
│       │       │                   # the find_project_root Tauri command
│       │       ├── script_filename_mismatch.rs # The "ScriptName/filename
│       │       │                   # mismatch" project lint (needs the .psc's
│       │       │                   # own path, so it can't live in
│       │       │                   # papyrus-lints with the source-only lints)
│       │       ├── source_encoding.rs # Reads a .psc as UTF-8 or, when that's
│       │       │                   # invalid, Windows-1252 (CP1252) — the
│       │       │                   # Creation Kit/compiler's own encoding
│       │       ├── script_locator.rs   # Finds .psc files by name under
│       │       │                       # scripts/source or source/scripts
│       │       ├── function_table/     # Cross-script function signature lookup,
│       │       │                       # for the argument/return type check lints
│       │       │                       # and script_exists() for the unresolved
│       │       │                       # script reference lint; each signature
│       │       │                       # tracks the State block it came from (if
│       │       │                       # any), preferring the empty state's own
│       │       │                       # declaration over a same-named override
│       │       │   ├── mod.rs          # FunctionTable struct and constructors
│       │       │   ├── tests.rs        # mod.rs's unit tests
│       │       │   ├── ancestry.rs     # Extends-chain lookups (functions,
│       │       │   │                   # properties, states, members)
│       │       │   ├── ancestry_tests.rs # ancestry.rs's unit tests
│       │       │   ├── load.rs         # Locate/parse/cache scripts on demand
│       │       │   ├── load_tests.rs   # load.rs's unit tests
│       │       │   ├── external.rs     # ExternalSignatures impl for FunctionTable
│       │       │   ├── external_tests.rs # external.rs's unit tests
│       │       │   ├── shared.rs       # RwLock SharedFunctionTable adapter
│       │       │                       # (write lock only on cache fill)
│       │       │   ├── shared_tests.rs # shared.rs's unit tests
│       │       │   └── test_support.rs # Shared helpers for this module's tests
│       │       ├── script_functions.rs # Converts a parsed .psc AST into the
│       │       │                       # FunctionSignature/PropertySignature/Member
│       │       │                       # types function_table looks up and
│       │       │                       # caches (and re-exports from there)
│       │       ├── native_globals.rs   # Known native singleton scripts (Game,
│       │       │                       # Utility, Debug, ...) always called by
│       │       │                       # literal name, with no .psc in the
│       │       │                       # project; reads shared/rules/data/native-globals.yaml
│       │       │                       # via a build-time-generated array (build.rs)
│       │       ├── presets.rs          # Label/description metadata for the desktop
│       │       │                       # app's first-run preset picker, layered over
│       │       │                       # papyrus_lint_config::presets::Preset (see
│       │       │                       # Configuration below)
│       │       ├── compiler.rs         # Runs PapyrusCompiler.exe for the desktop
│       │       │                       # app's "Compile" button, then strips personal
│       │       │                       # data from the compiled .pex; also compiles
│       │       │                       # into a throwaway temp dir (never touching
│       │       │                       # the project's real output) for the
│       │       │                       # compile_check lint setting, honored by both
│       │       │                       # the desktop app and the CLI, below
│       │       ├── compile_diagnostics.rs # Parses PapyrusCompiler.exe's own reported
│       │       │                          # errors, from compiler.rs's temp-dir
│       │       │                          # compile, into lint Diagnostics for the
│       │       │                          # compile_check setting
│       │       ├── pex_header.rs       # Parses a compiled .pex file's header just
│       │       │                       # far enough to blank its userName/
│       │       │                       # machineName fields
│       │       ├── parallel.rs         # rayon-backed worker pool (map_in_parallel)
│       │       │                       # spreading per-script work across threads,
│       │       │                       # used by the CLI's --threads flag
│       │       └── stale_pex.rs        # The "Stale compiled output" project lint:
│       │                               # flags a .psc file whose compiled .pex is
│       │                               # older than the script itself, a common
│       │                               # sign someone forgot to recompile after
│       │                               # editing it
│       ├── papyrus-lint-output/  # Plain-text/JSON/AI-export report
│       │   └── src/               # formatting, shared by papyrus-lint-cli
│       │       ├── lib.rs          # and the desktop app's Tauri commands
│       │       │                  # (app/src-tauri/src/export.rs) so a
│       │       │                  # diagnostic's exported shape can't drift
│       │       │                  # between the CLI and the GUI. Depends
│       │       │                  # only on papyrus-lints.
│       │       ├── diagnostic.rs   # DiagnosticLike trait (implemented by both
│       │       │                  # papyrus_lints::Diagnostic and this crate's
│       │       │                  # own OwnedDiagnostic, for a diagnostic that
│       │       │                  # arrived as plain JSON over Tauri's IPC
│       │       │                  # boundary), level_of, strip_severity_prefix
│       │       ├── json.rs         # JsonDiagnostic/JsonFileReport/JsonReport,
│       │       │                  # to_json_diagnostics, doc_url_for
│       │       ├── ai.rs           # AiReport and friends, build_ai_report,
│       │       │                  # ai_configuration
│       │       └── plain.rs        # format_diagnostic_line, ColorChoice/
│       │                          # resolve_color, colorize
│       └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-ppj-or-psc>`: lints
│           ├── src/                # an achlist's/ppj's scripts against its project's
│           │   ├── lib.rs           # run() dispatch + public API only; also
│           │   │                    # linked into src-tauri for its CLI mode
│           │   ├── args/            # Parses/validates run()'s arguments
│           │   │   ├── mod.rs         # parse.rs is clap's tree (init/preset/
│           │   │   ├── parse.rs       # doctor as subcommands, lint/fix/--blob
│           │   │   ├── validate.rs    # as the default command); validate.rs
│           │   │   └── help.rs        # applies lint/fix/--blob business rules
│           │   │                      # (mod.rs owns the shared types); help.rs
│           │   │                      # turns a rejected ArgsError into stderr
│           │   ├── run_lint_command.rs # Runs a parsed plain lint/fix
│           │   │                    # invocation end to end (run_scan +
│           │   │                    # run_fix/run_lint + report assembly)
│           │   ├── run_scan.rs      # Resolves the scripts a run targets and
│           │   │                    # the project state to lint/fix them against
│           │   ├── run_lint.rs      # Lints one already-resolved script source
│           │   ├── run_fix.rs       # Applies automatic fixes to one script,
│           │   │                    # before run_lint lints the result
│           │   ├── project.rs       # Project-root discovery from .psc paths,
│           │   │                    # is_psc_path/is_ppj_path, absolutize
│           │   ├── output/          # Plain/JSON/AI report types and formatting
│           │   ├── init.rs          # `init` / `preset add`: parsing, and the
│           │   │                    # run_init/run_preset_add entrypoints run()
│           │   │                    # dispatches straight to; also seeds
│           │   │                    # additional_script_roots from a .ppj
│           │   │                    # found next to a freshly initialized config
│           │   ├── blob.rs          # `--blob` in-memory lint
│           │   ├── doctor/          # `doctor` subcommand: mod.rs owns arg
│           │   │   ├── mod.rs         # parsing + orchestration and dispatches
│           │   │   ├── checks.rs      # to checks.rs (each individual health
│           │   │   └── report.rs      # check) and report.rs (its plain-text/
│           │   │                      # --json rendering)
│           │   ├── test_support.rs  # Shared helpers for each file's unit tests
│           │   ├── run_tests/       # Integration-style tests for run()'s
│           │   │   ├── mod.rs         # end-to-end pipeline, split by
│           │   │   ├── basic.rs       # scenario: plain lint/scan, fix,
│           │   │   ├── fix.rs         # --tag filtering, threaded vs.
│           │   │   ├── tag_filter.rs  # sequential parity, cross-script
│           │   │   ├── threading.rs   # resolution/strict achlist scope,
│           │   │   ├── cross_script_resolution.rs # stale-compiled-output/
│           │   │   ├── stale_output_and_filename_checks.rs # script-filename-
│           │   │   ├── script_roots_and_config.rs # mismatch/conflicting-
│           │   │   └── compile_check.rs # script-versions, script roots +
│           │   │                    # --config, and compile_check
│           │   └── main.rs          # Thin binary entry point around lib::run()
│           └── tests/               # Binary e2e tests, one file per src module
├── shared/
│   ├── images/               # Images used by README.md (logo, screenshots)
│   ├── links.yaml            # Contact/download/documentation URLs; labels
│   │                         # are the keys, `type` values are tags that
│   │                         # `<!--CONTACT-LINKS-->` / `<LINKS>` markers
│   │                         # filter by at build time
│   ├── rules/                 # One <id>.json per lint rule (the single
│   │   │                      # source of truth for rule metadata — see
│   │   │                      # AGENTS.md hard rule 4); rules.json (the
│   │   │                      # combined array every other consumer reads)
│   │   │                      # is generated from these by
│   │   │                      # .github/scripts/build_rules_json.py and is
│   │   │                      # git-ignored, not checked in
│   │   └── data/             # YAML lookup tables compiled into the Rust crates:
│   │       ├── forbidden-functions.yaml # Discouraged/forbidden calls
│   │       ├── slow-functions.yaml      # Slow calls and faster alternatives
│   │       ├── native-methods.yaml      # Base-game native functions
│   │       ├── update-event-handlers.yaml # RegisterFor*/Event pairs
│   │       ├── known-events.yaml        # Curated native Event signatures
│   │       ├── native-globals.yaml      # Native singleton scripts
│   │       └── actor-values.yaml        # Skyrim's built-in Actor Values
│   └── theme.css             # Palette, canvas, and primitives shared by
│                              # app/src/styles.css and pages/styles.css so
│                              # the desktop app and the website cannot drift
├── SublimeLinter-contrib-papyrus-lint/  # Standalone SublimeLinter plugin package,
│   ├── linter.py                          # runs PapyrusLinterCLI against a saved
│   ├── messages.json                      # .psc file and parses its output
│   ├── messages/install.txt               # into SublimeLinter diagnostics; kept
│   ├── README.md                          # here for development but installed/
│   └── LICENSE                            # distributed as its own package.
├── vscode-extension/        # VS Code extension (TypeScript): lints and fixes
│   ├── package.json          # .psc files by invoking PapyrusLinterCLI --json
│   └── src/
│       ├── extension.ts      # activate/deactivate: wires listeners and commands
│       ├── config.ts         # papyrusLint.* settings
│       ├── cli.ts            # spawn PapyrusLinterCLI / auto-download cache
│       ├── cliDownload.ts    # GitHub release CLI download
│       ├── diagnostics.ts    # JSON report parse/normalize (no vscode import)
│       ├── vscodeDiagnostics.ts # JSON → vscode.Diagnostic
│       ├── documents.ts      # .psc/Papyrus document predicates + target URI
│       ├── linter.ts         # PapyrusLinter: lint / blob / fix / applyResult
│       ├── liveLint.ts       # debounced as-you-type --blob lint
│       ├── init.ts           # papyrusLint.initializeConfig wizard
│       ├── suppressions.ts   # @disable / @disable-file / papyrus-lint.yaml rule-off edits
│       ├── ignore.ts         # "Ignore this lint for the line/file/project" commands
│       └── codeActions.ts    # "Fix this issue" / ignore CodeActionProvider
│   └── test/                 # Node-based extension unit tests (per-module)
├── pages/                   # Source for the GitHub Pages discoverability site
│   ├── index.template.html    # (see GitHub Pages below): index.template.html is
│   ├── docs.template.html      # styled to match the desktop app's frontend (Cinzel
│   ├── videos.template.html    # headings, the same light/dark palette); build.py
│   ├── videos.json             # substitutes its lint-table/CLI-example placeholders
│   ├── action.template.html    # and renders action.html, the papyrus-lint-action
│   │                            # GitHub Action's own README fetched at build
│   │                            # time (see action.template.html below).
│   ├── coverage.template.html  # with content converted straight from README.md,
│   ├── imprint.template.html   # renders imprint.html, a fully static legal
│   │                            # notice (Impressum) with no build-time
│   │                            # content of its own beyond the shared header/
│   │                            # footer, linked from the footer on every page
│   ├── rules.template.html     # renders rules.html, a searchable/filterable
│   ├── rules.js                 # reference of every lint rule generated from
│   │                            # shared/rules.json's own metadata (id, severity,
│   │                            # tags, fixable, full definition); rules.js
│   │                            # (minified into the output directory like
│   │                            # downloads.js) wires up its search box and
│   │                            # severity/tag/auto-fix checkboxes
│   ├── includes/               # shared page chrome inserted during the build
│   │   ├── header.html         # with depth-aware links for root/docs pages
│   │   └── footer.html         # and one source for release/contact/legal
│   │                            # notice details
│   ├── styles.css              # Site layout/components; imports
│   │                            # shared/theme.css for the palette/canvas.
│   │                            # build.py inlines that import (and minifies)
│   │                            # so the deployed site is still one file.
│   ├── CNAME                   # The site's custom domain (papyrus-lint.idrinth.de);
│   │                            # build.py copies CNAME into pages/dist/ so
│   │                            # GitHub Pages keeps serving it across every
│   │                            # Actions-based deploy.
│   ├── fonts/                  # renders every docs/* file into a browsable subpage
│   │   ├── cinzel-v26-latin-700.woff2  # (via docs.template.html) linked from a
│   │   └── inter-v20-latin-variable.woff2  # Documentation section, renders
│   ├── build.py                # videos.json's list of YouTube videos into
│   │                            # videos.html (via videos.template.html), a
│   │                            # --coverage-dir of downloaded lcov reports into
│   │                            # coverage.html (via coverage.template.html, see
│   │                            # GitHub Pages below), and assembles pages/dist/
│   │                            # (git-ignored), copying
│   │                            # its assets/ images from shared/images/ and the
│   │                            # app icon rather than committing duplicates of
│   │                            # either under pages/, generating a WebP/AVIF
│   │                            # sibling of each one rendered as an <img> and
│   │                            # rewriting that <img> into a <picture> offering
│   │                            # them (see GitHub Pages below), its fonts/
│   │                            # woff2 files as-is so styles.css's @font-face
│   │                            # rules self-host Cinzel/Inter instead of
│   │                            # pulling them from
│   │                            # fonts.googleapis.com/fonts.gstatic.com
│   │                            # (avoiding a third-party request on every page
│   │                            # load), and a sitemap.xml/robots.txt pair (see
│   │                            # GitHub Pages below) rooted at SITE_URL.
│   ├── requirements-build.txt  # Pinned Pillow version build.py's image
│   │                            # conversion above depends on.
│   ├── browser_check.py        # Opens every page under a built pages/dist in
│   │                            # headless Chromium (see CI below) to catch
│   │                            # console/page errors and broken internal
│   │                            # links/anchors that build.py's own unit
│   │                            # tests, working against small fixtures, can't
│   └── requirements-browser-check.txt  # Pinned Playwright version for the above
└── docker/                  # Release Alpine CLI image (ghcr.io/idrinth/papyrus-lint)
    ├── Dockerfile              # Installs PapyrusLinterCLI, generates the
    │                            # image's --preset (build ARG) config, and
    │                            # unpacks the bundled base-scripts archive
    ├── entrypoint.sh            # Resolves /project, /cache, /base-scripts (or
    │                            # PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE) and runs
    │                            # PapyrusLinterCLI against them
    └── Scripts.zip              # Bundled Skyrim SE base scripts for cross-script
                                   # lookups when no base-scripts volume is mounted
```

`papyrus-parser`, `papyrus-ast-cache`, `papyrus-lints`, `papyrus-lint-config`,
`papyrus-lint-core`, `papyrus-lint-output`, and `papyrus-lint-cli` are separate
crates (not Cargo workspace members, just path dependencies of each other and of
`app/src-tauri`) so the lint engine and project-resolution logic stay reusable
independent of the Tauri app — which is what lets `papyrus-lint-cli` link against
them without pulling in Tauri (and its system GUI dependencies) at all. `app/src-tauri`
depends on `papyrus-lint-cli` too, purely for its `run()` function (its
`main.rs` calls straight into it for CLI mode), not for the `PapyrusLinterCLI`
binary target that crate also defines. `app/src-tauri` also depends on
`papyrus-lint-output` directly, for its own `format_issues_as_text`/
`format_issues_as_json`/`format_issues_for_ai_base` Tauri commands (see
`app/src-tauri/src/export.rs`) backing the desktop app's "Export issues"/
"Export for AI" buttons — built on the same plain-text/JSON/AI-export
formatting `papyrus-lint-cli`'s `--json`/`--format ai` flags use, so the
GUI and CLI's exported report shapes can't drift apart. The GUI's own
extras beyond that shared shape (the currently active result filters, an
`external: true`/`source: "compiler"` tag on a compiler-reported
diagnostic, and a per-diagnostic repair preview) are layered on top of
that command's output by `app/src/results-export-ai.ts`, which needs data
(live filter state, an async repair-preview lookup per finding) the Tauri
command itself has no way to obtain.

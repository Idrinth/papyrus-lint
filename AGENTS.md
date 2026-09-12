# Papyrus Lint

A linter for Bethesda's Papyrus scripting language, packaged as a desktop
app and a CLI. The desktop app is a Tauri (Rust + TypeScript) app: the
frontend lets a user drop a `.achlist` file, or a single `.psc` file
directly, and the Rust backend resolves the achlist's listed files (or
just the single dropped `.psc`), parses whichever `.psc` (Papyrus source)
files result, and lints them. The CLI (`app/crates/papyrus-lint-cli`) does the same thing
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
│   ├── e2e/                  # Playwright specs (real Chromium, not jsdom):
│   │   └── layout.spec.ts       # catches element-size/layout regressions
│   ├── playwright.config.ts  # Config for the e2e/ specs above
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
│   │       │                     # then strips personal data from the compiled .pex;
│   │       │                     # also compiles into a throwaway temp dir (never
│   │       │                     # touching the project's real output) for the
│   │       │                     # compile_check lint setting, below
│   │       ├── compile_diagnostics.rs # Parses PapyrusCompiler.exe's own reported
│   │       │                     # errors, from compiler.rs's temp-dir compile, into
│   │       │                     # lint Diagnostics for the compile_check setting
│   │       └── pex_header.rs      # Parses a compiled .pex file's header just far
│   │                             # enough to blank its userName/machineName fields
│   └── crates/
│       ├── papyrus-parser/       # Standalone Rust crate: lexer, AST, and parser
│       │   └── src/               # for the Papyrus language. No lint rules live
│       │       ├── lexer.rs        # here — see papyrus-lints below.
│       │       ├── token.rs
│       │       ├── ast.rs
│       │       ├── parser.rs
│       │       └── cache.rs        # In-memory memoization of parse()/tokenize()
│       │                           # against the most recently seen source, so
│       │                           # one lint pass over a script only lexes/
│       │                           # parses it once no matter how many lint
│       │                           # rules each ask for their own tokens/AST
│       ├── papyrus-lints/        # Lint rules, each inspecting raw source/tokens
│       │   ├── build.rs           # (not the AST) so they still run on scripts
│       │   └── src/                # that don't parse cleanly.
│       │       ├── lib.rs                     # Diagnostic type + lint()/repair() entry points
│       │       ├── config.rs                  # Config type (YAML-deserializable) passed
│       │       │                              # to every check/fix job
│       │       ├── trailing_whitespace.rs     # Flags trailing spaces/tabs per line
│       │       ├── forbidden_functions.rs     # Reads rules/forbidden-functions.yaml
│       │       │                              # via a build-time-generated array
│       │       └── native_function_usage.rs   # Reads rules/native-methods.yaml via a
│       │                                      # build-time-generated array; disabled by
│       │                                      # default
│       ├── papyrus-lint-core/    # Project-level logic shared by the desktop app
│       │   └── src/               # and the CLI, independent of Tauri:
│       │       ├── achlist.rs      # Parses .achlist files (JSON arrays of paths)
│       │       ├── ast_cache.rs    # Disk-backed cache of parsed .psc ASTs, keyed by
│       │       │                   # content MD5 + mtime + linter version, shared by
│       │       │                   # the desktop app and the CLI (via function_table.rs)
│       │       ├── config.rs       # Locates/loads a project's papyrus-lint.yaml
│       │       ├── script_locator.rs   # Finds .psc files by name under
│       │       │                       # scripts/source or source/scripts
│       │       ├── function_table.rs   # Cross-script function signature lookup,
│       │       │                       # for the argument/return type check lints
│       │       │                       # and script_exists() for the unresolved
│       │       │                       # script reference lint; each signature
│       │       │                       # tracks the State block it came from (if
│       │       │                       # any), preferring the empty state's own
│       │       │                       # declaration over a same-named override
│       │       ├── native_types.rs     # Fallback Extends hierarchy for native
│       │       │                       # engine types (Actor, ObjectReference,
│       │       │                       # Form, ...) with no .psc in the project;
│       │       │                       # reads rules/native-types.yaml via a
│       │       │                       # build-time-generated array (build.rs)
│       │       ├── native_globals.rs   # Known native singleton scripts (Game,
│       │       │                       # Utility, Debug, ...) always called by
│       │       │                       # literal name, with no .psc in the
│       │       │                       # project; reads rules/native-globals.yaml
│       │       │                       # via a build-time-generated array (build.rs)
│       │       └── presets.rs          # Label/description metadata for the desktop
│       │                               # app's first-run preset picker, layered over
│       │                               # config::Preset (see Configuration below)
│       └── papyrus-lint-cli/     # `PapyrusLinterCLI <achlist-or-psc>`: lints an
│           └── src/                # achlist's scripts against its project's
│               ├── lib.rs           # papyrus-lint.yaml and prints the results.
│               │                    # run() here is the shared logic; also
│               │                    # linked into src-tauri for its CLI mode.
│               └── main.rs          # Thin binary entry point around lib::run()
├── resources/                # Images used by README.md (logo, screenshots)
├── rules/
│   ├── forbidden-functions.yaml  # Calls discouraged or forbidden by policy
│   ├── slow-functions.yaml       # Slow calls and their faster alternatives
│   ├── native-methods.yaml       # Base-game native functions (see
│   │                             # native_function_usage.rs above); all three
│   │                             # files above are compiled in by
│   │                             # papyrus-lints/build.rs
│   ├── native-types.yaml         # Native engine class hierarchy fallback (see
│   │                              # papyrus-lint-core/src/native_types.rs above);
│   │                              # compiled in by papyrus-lint-core/build.rs
│   └── native-globals.yaml       # Native singleton scripts always called by
│                                  # literal name (see native_globals.rs above);
│                                  # compiled in by papyrus-lint-core/build.rs
├── SublimeLinter-contrib-papyrus-lint/  # Standalone SublimeLinter plugin package,
│   ├── linter.py                          # runs PapyrusLinterCLI against a saved
│   ├── messages.json                      # .psc file and parses its output
│   ├── messages/install.txt               # into SublimeLinter diagnostics; kept
│   ├── README.md                          # here for development but installed/
│   └── LICENSE                            # distributed as its own package.
├── vscode-extension/        # VS Code extension (TypeScript): lints and fixes
│   ├── package.json          # .psc files by invoking PapyrusLinterCLI --json
│   ├── src/extension.ts      # Commands, process execution, and diagnostics
│   └── test/                 # Node-based extension unit tests
└── pages/                   # Source for the GitHub Pages discoverability site
    ├── index.template.html    # (see GitHub Pages below): index.template.html is
    ├── docs.template.html      # styled to match the desktop app's frontend (Cinzel
    ├── videos.template.html    # headings, the same light/dark palette); build.py
    ├── videos.json             # substitutes its lint-table/CLI-example placeholders
    ├── action.template.html    # and renders action.html, the papyrus-lint-action
    │                            # GitHub Action's own README fetched at build
    │                            # time (see action.template.html below).
    ├── coverage.template.html  # with content converted straight from README.md,
    ├── imprint.template.html   # renders imprint.html, a fully static legal
    │                            # notice (Impressum) with no build-time
    │                            # content of its own beyond the shared header/
    │                            # footer, linked from the footer on every page
    ├── includes/               # shared page chrome inserted during the build
    │   ├── header.html         # with depth-aware links for root/docs pages
    │   └── footer.html         # and one source for release/contact/legal
    │                            # notice details
    ├── styles.css              # and renders coverage.html, a per-module/per-file
    ├── CNAME                   # line coverage breakdown for the latest release
    │                            # (see coverage.template.html below). The site's
    │                            # custom domain (papyrus-lint.idrinth.de);
    │                            # build.py copies CNAME into pages/dist/ so
    │                            # GitHub Pages keeps serving it across every
    │                            # Actions-based deploy.
    ├── fonts/                  # renders every docs/* file into a browsable subpage
    │   ├── cinzel-v26-latin-700.woff2  # (via docs.template.html) linked from a
    │   └── inter-v20-latin-variable.woff2  # Documentation section, renders
    ├── build.py                # videos.json's list of YouTube videos into
    │                            # videos.html (via videos.template.html), a
    │                            # --coverage-dir of downloaded lcov reports into
    │                            # coverage.html (via coverage.template.html, see
    │                            # GitHub Pages below), and assembles pages/dist/
    │                            # (git-ignored), copying
    │                            # its assets/ images from resources/ and the
    │                            # app icon rather than committing duplicates of
    │                            # either under pages/, generating a WebP/AVIF
    │                            # sibling of each one rendered as an <img> and
    │                            # rewriting that <img> into a <picture> offering
    │                            # them (see GitHub Pages below), its fonts/
    │                            # woff2 files as-is so styles.css's @font-face
    │                            # rules self-host Cinzel/Inter instead of
    │                            # pulling them from
    │                            # fonts.googleapis.com/fonts.gstatic.com
    │                            # (avoiding a third-party request on every page
    │                            # load), and a sitemap.xml/robots.txt pair (see
    │                            # GitHub Pages below) rooted at SITE_URL.
    ├── requirements-build.txt  # Pinned Pillow version build.py's image
    │                            # conversion above depends on.
    ├── browser_check.py        # Opens every page under a built pages/dist in
    │                            # headless Chromium (see CI below) to catch
    │                            # console/page errors and broken internal
    │                            # links/anchors that build.py's own unit
    │                            # tests, working against small fixtures, can't
    └── requirements-browser-check.txt  # Pinned Playwright version for the above
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
  recommended rules on test files. `npm run lint:css` runs stylelint (config
  in `.stylelintrc.json`, extending `stylelint-config-recommended`) over
  `src/**/*.css`. `npm run test:browser` runs `app/e2e/*.spec.ts` (config in
  `playwright.config.ts`) against a real Chromium instance (via
  `@playwright/test`, browsers installed separately with `npx playwright
  install --with-deps chromium`) rather than jsdom, starting the Vite dev
  server itself: jsdom (used by `npm run test` above) never computes an
  actual box model, so it can't catch element-size/layout regressions
  (a collapsed drop zone, a mis-hidden tab panel, an overlay no longer
  matching its underlying element's dimensions, horizontal overflow) the
  way these tests do.
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

- **Pull request labels job** (`labels`): on a pull request, fails unless
  the pull request carries at least one `component: ...` label and at
  least one `type: ...` label (see Pull request labels below), via
  `actions/github-script` reading `context.payload.pull_request.labels`
  directly rather than calling the API. It's a no-op on an ordinary push
  to `the-one`, since that event carries no pull request labels to check.
  Every other job `needs` this one (directly, or transitively through
  `rust-fmt`/`rust-clippy`/`rust-test`), so an unlabeled pull request's CI
  stops here instead of spending time on the rest of the jobs below.
- **Rules YAML lint job**: runs `yamllint` against every `rules/*.yaml` file
  so malformed rule data cannot be merged.
- **Python lint job** (`python-lint`): runs `ruff check` (configured in the
  root `pyproject.toml`) against every Python source under `.github/scripts`,
  `pages`, and `SublimeLinter-contrib-papyrus-lint`.
- **CI scripts and Nexus page BBCode job**: runs the dependency-free Python
  scripts' unit tests with coverage, publishes the text summary, uploads the
  lcov report as the `ci-scripts-coverage` artifact, then checks
  `docs/nexuspage.bbcode` for unknown, mismatched, and unclosed tags.
- **Semantic version advisory job** (`semver-advisory`, pushes to `the-one`
  only): gathers every pull request merged since the latest `v*.*.*` release
  tag (via the GitHub API, walking `git log <tag>..HEAD`) and passes their
  labels to `.github/scripts/semver_advisory.py`, which recommends the next
  semantic version to tag based on the `type: *` labels below — highest
  precedence wins across all of them: any `type: breaking change` recommends
  a major bump, else any `type: feature` recommends a minor bump, else any
  `type: refactoring`/`type: tests`/`type: documentation` recommends a patch
  bump. A pull request with none of those labels contributes nothing. This
  is overridden for a pull request whose `component: *` label(s) are
  exclusively among `component: ci`, `component: pages`, and
  `component: documentation`: it always recommends a patch bump instead,
  regardless of its `type: *` label(s) (if any), since none of those three
  components reach the end user on their own. The
  recommendation (a per-pull-request table plus the suggested next version)
  is posted to the job's step summary; the job itself never edits a file
  or fails the job — the actual release still only happens when a
  maintainer pushes a `v*.*.*` tag (see Releases below). It does, however,
  create or update a draft GitHub release for the recommended version so
  a maintainer has something to review and tweak in the meantime: it
  looks up any existing draft release whose body carries a
  `<!-- semver-advisory: auto-generated draft -->` marker (never touching
  a draft a maintainer created by hand, which carries no such marker),
  and either updates that draft's title/notes/target commit in place
  (recommendation unchanged since the last push — the target commit is
  re-pointed at the current push's SHA every time, not just on creation,
  so the draft never lags behind and a maintainer can't accidentally
  publish it against a stale, hours-old commit), deletes and recreates it
  under the new tag (recommendation changed, e.g. a later push adds a
  `type: breaking change` label), or creates a fresh one (no existing draft) via
  `semver_advisory.py`'s own `--release-notes`/`--outputs` output. A
  draft release carries no real git tag until it's published — GitHub
  only creates the tag then — so this never triggers `release.yml` or
  otherwise acts on the recommendation itself; publishing (or discarding)
  the draft is still a maintainer's call. When no pull request carries a
  recognized `type: *` label, no bump is recommended and any existing
  auto-generated draft is left untouched. The draft's notes also carry a
  "Test coverage" section, so a maintainer can gauge how well tested the
  codebase currently is before deciding whether to actually cut that
  release: the job looks up the most recent successful `ci.yml` run on
  `the-one` (excluding this in-progress run itself, which is what
  `status=completed` filters out, so this is the latest run whose
  coverage jobs actually finished — a commit or two behind `HEAD` rather
  than tied to this exact push), downloads its `*coverage*` artifacts the
  same way `release.yml`'s `release-notes` job does for a tagged release,
  and aggregates them with the same `.github/scripts/coverage_summary.py`
  into a Markdown table passed to `semver_advisory.py`'s
  `--coverage-summary` flag, which folds it into the generated release
  notes (`build_release_notes`) right after the pull request list. No
  successful run found yet just renders as a "coverage data unavailable"
  placeholder instead of failing the job, the same fallback
  `pages/build.py`'s `coverage.html` uses.
- **GitHub Pages browser smoke test job**: builds the site
  (`pages/build.py`) and then opens every one of its pages in headless
  Chromium via Playwright (`pages/browser_check.py`) to catch what
  `pages`' own unit tests can't, since those only exercise `build.py`'s
  Markdown/table conversion logic against small in-memory fixtures rather
  than the actual rendered output: JavaScript console/page errors,
  same-origin resources that fail to load or respond with a 4xx/5xx
  status, and internal links (including `#fragment` anchors) that point
  at a page or in-page id that doesn't actually exist. External links
  (GitHub, Discord, Nexus Mods, badge/font hosts, ...) are faked with a
  harmless empty response rather than fetched, so the check stays fast
  and doesn't depend on services this repository doesn't control.
- **GitHub Pages Lighthouse check job** (`pages-lighthouse`, pull requests
  only): builds the site (`pages/build.py`), serves it locally, and runs
  the `lighthouse` CLI (installed via npm, against the runner's
  preinstalled Chrome) over every one of its pages for the performance,
  accessibility, best-practices, and SEO categories, writing each page's
  JSON/HTML report into a `lighthouse-reports` artifact. A per-page audit
  failure is logged as a workflow warning and fails the job, without
  stopping the remaining pages from being audited.
  `.github/scripts/lighthouse_summary.py` turns the JSON reports into a
  Markdown table of category scores plus, for any category scoring under
  90/100, the specific audits behind that score, posted as a single PR
  comment (updated in place on subsequent pushes, the same
  marker-comment approach as the coverage summary comment below) and to
  the job's step summary. Comment posting is best-effort
  (`continue-on-error`) since forked PRs get a read-only `GITHUB_TOKEN`.
- **Sublime Text extension job**: runs the plugin's Python unit tests via
  `coverage run -m unittest discover`, scoped to `commands.py`/`linter.py`
  (test files themselves are omitted). The text summary is posted to the
  job's step summary and an lcov report is uploaded as the
  `sublime-extension-coverage` artifact.
- **Frontend CSS lint job**: in `app/`, `npm ci`, then `npm run lint:css`
  (stylelint) over `src/**/*.css`.
- **Frontend job**: in `app/`, `npm ci`, then `npm run lint` (ESLint), `npm
  run test:coverage` (Vitest unit tests, instrumented for coverage), and `npm
  run build` (typecheck & Vite build). The text coverage summary is
  posted to the job's step summary and the full HTML/lcov report is
  uploaded as the `frontend-coverage` artifact.
- **Frontend browser job**: in `app/`, `npm ci`, `npx playwright install
  --with-deps chromium`, then `npm run test:browser` (see Development
  above) to catch element-size/layout regressions a real browser renders
  but jsdom can't. On failure, the HTML report is uploaded as the
  `playwright-report` artifact.
- **Frontend Lighthouse check job** (`app-lighthouse`, pull requests only):
  builds the frontend (`npm run build`), serves `app/dist` locally, and
  runs the same `lighthouse` CLI as the GitHub Pages Lighthouse check job
  above over its page(s), for the performance, accessibility, and
  best-practices categories only — unlike the GitHub Pages job, it skips
  SEO, since the desktop app's frontend is never crawled or indexed as a
  public web page — writing each page's JSON/HTML report into an
  `app-lighthouse-reports` artifact. A per-page audit failure is logged as
  a workflow warning and fails the job, without stopping the remaining
  pages from being audited. It calls the same
  `.github/scripts/lighthouse_summary.py` with a second `App` argument, so
  its Markdown summary (posted to the job's step summary and as an
  updated-in-place PR comment, same as above) uses its own marker/title,
  omits the SEO column entirely (`lighthouse_summary.py`'s
  `active_categories` renders only the categories actually present in the
  loaded reports), and never overwrites the GitHub Pages job's comment.
  Comment posting is best-effort (`continue-on-error`) since forked PRs
  get a read-only `GITHUB_TOKEN`.
- **Markdown job**: runs markdownlint-cli2 against every `README.md` in the
  repository, using the root `.markdownlint-cli2.yaml` configuration.
- **VS Code extension job**: installs its dependencies, then runs `npm run
  test:coverage` (the Node test runner's built-in coverage, via
  `--experimental-test-coverage`), ESLint, and TypeScript compilation. The
  text coverage summary is posted to the job's step summary and an lcov
  report is uploaded as the `vscode-extension-coverage` artifact.
- **Rust fmt job** (`rust-fmt`): runs `cargo fmt --check` against every
  crate's own `Cargo.toml` (`app/src-tauri` and all four reusable crates
  under `app/crates`) — not just `app/src-tauri` — since they're separate
  crates rather than workspace members and so aren't formatted together
  by a single invocation. It needs none of `rust-clippy`'s Tauri system
  dependencies or build cache, since checking formatting never compiles
  anything, so it runs in parallel with `rust-clippy` instead of after it.
- **Rust clippy job** (`rust-clippy`): runs `cargo clippy --all-targets --
  -D warnings` against `app/src-tauri/Cargo.toml`. Runs in parallel with
  `rust-fmt` (both only `need` the `labels` job); `rust-test` (below)
  `needs` both.
- **Rust test job**: a matrix over `app/src-tauri`, `app/crates/papyrus-parser`,
  `app/crates/papyrus-lints`, `app/crates/papyrus-lint-core`, and
  `app/crates/papyrus-lint-cli` runs each crate's tests via `cargo llvm-cov`.
  Each matrix leg posts its text coverage summary to the job's step
  summary and uploads its lcov report as a `rust-coverage-<crate>`
  artifact.
- **Coverage summary comment job** (`coverage-comment`, pull requests
  only): downloads every job's lcov artifact and runs
  `.github/scripts/coverage_summary.py` to aggregate line coverage by
  module — App (`src-tauri`, the frontend, and Crates — the four reusable
  crates combined, nested underneath it), editor plugins (the VS Code
  extension and the Sublime Text plugin combined), and Tooling (CI tooling
  and the Pages builder) — posting the result as a single markdown table,
  with groups and their entries sorted alphabetically, updated in place on
  subsequent pushes, as a PR comment (and to the job's step summary).
  Comment posting is best-effort (`continue-on-error`) since forked PRs get
  a read-only `GITHUB_TOKEN`.

Note: CI runs on pushes to `the-one` (the default branch, not `main`) and
on all pull requests.

## GitHub Pages (`.github/workflows/pages.yml`, `pages/build.py`)

A manual `workflow_dispatch` run (including one fired remotely by
`release.yml`'s `update-pages` job, see Releases below) builds and deploys
a discoverability landing page to GitHub Pages. It deliberately has no
`push` trigger of its own: the site's footer always shows a specific
released version (see "Determine version" below), and a push to `the-one`
that merely touches `pages/**`/`README.md`/`docs/**`/etc. can land content
— a lint table edit, a new doc, an in-progress `pages/` tweak — that isn't
part of that released version yet. Deploying on every such push would
publish that unreleased content immediately under the still-current
release's version number, misrepresenting what that version actually
contains. Instead, the site only ever redeploys when `update-pages`
explicitly asks it to, right after a release tag's own assets are built,
so the deployed content matches what that just-tagged version actually
contains rather than whatever has since landed on `the-one`. The
repository's Pages source must be set to "GitHub
Actions" (Settings → Pages) for this workflow to publish successfully.
The workflow also declares a `workflow_call` trigger with the same
`version` input as `workflow_dispatch`, but nothing actually calls it
that way (see `update-pages` below for why); it's kept only in case a
future in-repo caller wants to invoke it directly on `the-one` without a
network round trip through the GitHub API.

The page's footer displays the current version via a `<!--VERSION-->`
placeholder that `pages/build.py --version <tag>` fills in the same way
as the lint tables/CLI examples below. The workflow resolves that
version itself before building: it takes the caller-supplied `version`
input if `workflow_call`/`workflow_dispatch` provided one, otherwise
falls back to querying `gh release view` for the repository's latest
release tag; if neither resolves (e.g. no release exists yet), the page
shows "unreleased". A manual `workflow_dispatch` run with no `version`
input (e.g. to preview an already-released site locally-triggered from
the Actions tab) falls back the same way, while `release.yml`'s
`update-pages` job pins it explicitly to the tag it just built.

The same resolved version also drives the `coverage.html` subpage (see
`pages/coverage.template.html` above): the `deploy` job resolves that
version's commit SHA via the GitHub API, looks up the most recent
successful `ci.yml` run for that commit the same way `release.yml`'s
`release-notes` job does, and — only if one is found — downloads its
`*coverage*` artifacts into a local `coverage-artifacts` directory passed
to `pages/build.py --coverage-dir`. No version resolved, or no successful
CI run found for it, just means an empty `--coverage-dir` argument, which
`build.py` treats as "no coverage data" rather than a build failure. This
needed no separate CI job of its own: it's a few extra steps in the
existing `deploy` job, gated by `actions: read` alongside the permissions
that job already carries.

`pages/index.template.html` is a plain HTML/CSS page (no frontend
framework or bundler) styled to match the desktop app's frontend
(`app/src/styles.css`): the same Cinzel-headed, light/dark-aware
palette, including the same System/Light/Dark theme switch. The
`<select id="theme-select">` control itself lives once in
`pages/includes/header.html` (see GitHub Pages below), so it renders in
every page's header; `pages/theme.js` (copied verbatim into the built
site) wires it up, and a small blocking inline script duplicated into
each template's own `<head>`, ahead of its `<link rel="stylesheet">`,
applies an already-persisted light/dark override before first paint to
avoid a flash of the wrong theme — the same approach as
`app/index.html`'s own inline script. Both read/write the same
`papyrus-lint:theme` `localStorage` key and `data-theme` root-element
attribute scheme as the desktop app's theme switch
(`applyTheme`/`loadStoredTheme` in `app/src/main.ts`), so
`pages/styles.css`'s dark-mode rules mirror `app/src/styles.css`'s own
`:root:not([data-theme="light"])`/`:root[data-theme="light"]`/
`:root[data-theme="dark"]` structure. Rather than hand-duplicating the README's lint tables and CLI
usage examples into that template (and having to keep them in sync by
hand), it carries `<!--LINT_TABLE:Formatting-->`-style placeholder
comments — one per lint category listed in the README's [Implemented
Lints](README.md#implemented-lints) table, plus `<!--CLI_EXAMPLES-->` —
that `pages/build.py` fills in at build time by extracting and
converting the corresponding Markdown table/code block straight out of
`README.md`, so that content can never drift out of sync. It also
assembles the page's `assets/` directory by copying the screenshots
from `resources/` and the app icon from `app/src-tauri/icons/icon.png`,
rather than committing duplicate copies of them under `pages/`. For the
assets actually rendered as `<img>` elements (the header logo and the
five screenshots — not `logo.jpg`, only ever referenced as a raw
`og:image`/`twitter:image` URL, and not the favicon, only ever
referenced via `<link rel="icon">`), it additionally writes a WebP and
an AVIF sibling next to the copied original (`convert_to_modern_formats`,
via `Pillow` — see `pages/requirements-build.txt`), losslessly for the
PNG screenshots (so text/lines stay crisp) and lossy for the already-lossy
JPEG logo; `wrap_images_with_modern_sources` then rewrites every such
`<img>` tag, in every page this builder renders, into a `<picture>`
offering those two smaller formats as preferred `<source>`s ahead of the
original as the final fallback. Both the
workflow and a contributor previewing the page locally run it as
`python3 pages/build.py --out pages/dist` (the default `--out`), after
`pip install -r pages/requirements-build.txt`; its
output directory (`pages/dist` by default) is git-ignored (matched by
the root `.gitignore`'s generic `dist` entry) and gets uploaded to Pages
via `actions/upload-pages-artifact`/`actions/deploy-pages`. Everything
in `index.template.html` outside those placeholders — the hero pitch,
the "what this is/isn't" cards, screenshots, editor integrations, "how
to help" — is short, hand-authored prose kept in sync with `README.md`
by hand, the same way `docs/nexuspage.bbcode`'s own intro prose is (see
"Keeping agent instructions synchronized" below).

The site is served from the custom domain `papyrus-lint.idrinth.de`, read
from the checked-in `pages/CNAME` (just that domain, on its own line) at
build time into `build.py`'s `SITE_URL` constant, which every page's
`og:url`/`og:image`/`twitter:image` tags are built from (via the
`<!--SITE_URL-->` placeholder each template carries, filled in by
`render_shared_components`) rather than hardcoding the domain a second
time; `build.py` also copies `pages/CNAME` itself into `pages/dist/CNAME`
on every build so GitHub Pages keeps serving it there across each
Actions-based deploy, rather than relying solely on the custom-domain
setting under Settings → Pages. `build.py` also writes a `sitemap.xml`
(`sitemap_urls`/`build_sitemap`, rooted at `SITE_URL`) listing the
homepage, `action.html`, `videos.html`, `coverage.html`, `imprint.html`,
`docs/index.html`,
and every `DOCS` entry's own subpage — built from the same lists that
generate those pages, so it can't drift out of sync with what's actually
published — and a `robots.txt` (`build_robots_txt`) allowing all crawling
and pointing at that sitemap.

Every file in the `docs/` directory (see Project structure above) is also
published as its own browsable subpage, so that reference material isn't
only reachable as raw source on GitHub. `pages/build.py`'s `DOCS` list names
each local file or remote `content_url`, a `slug` for its output filename, and a `kind`
(`markdown`, `json-schema`, or plain text) that picks how it's rendered:
a Markdown document is converted to HTML the same way
the CLI examples are (headings, paragraphs, fenced code blocks, and
`render_inline`'s inline formatting), with its own top-level heading and
first paragraph read back out as the subpage's title/description rather
than duplicated in `DOCS`; a JSON Schema file renders its
`title`/`description` fields plus the pretty-printed schema itself in a
code block; anything else (`docs/papyrus-lint.default.yaml`,
`docs/nexuspage.bbcode`) renders as a plain code block under a
hand-written title/description in `DOCS`. A link inside a rendered
Markdown doc to another published doc (matched by filename) resolves to
that doc's own subpage; a `../`-relative link into the rest of the
repository resolves on GitHub instead — both via `resolve_doc_href`,
so `docs/github-actions-example.md`'s existing relative links keep
working once rendered. `pages/docs.template.html` is the page template
these subpages (and their `docs/index.html` listing) render into, carrying
its own `<!--DOC_TITLE-->`/`<!--DOC_DESCRIPTION-->`/`<!--DOC_CONTENT-->`
placeholders. All HTML page templates carry `<!--SITE_HEADER-->` and
`<!--SITE_FOOTER-->` markers which `render_shared_components` fills from
`pages/includes/header.html` and `pages/includes/footer.html`; its
depth-aware `<!--ROOT_PATH-->` replacement keeps links correct from both
the site root and `docs/`, while the footer's `<!--VERSION-->` is filled
from the same build argument everywhere. `index.template.html`'s own
`<!--DOCS_LIST-->` placeholder is filled with the same titles and a short
hand-written blurb per doc from `DOCS`, linking into `pages/dist/docs/`.
Adding a new file under `docs/` that should be published this way means
adding an entry to `DOCS`, not touching either template.

The [`papyrus-lint-action`](https://github.com/idrinth/papyrus-lint-action)
repository's own `README.md` is fetched from its `the-one` branch during
every site build, so that its GitHub Action's inputs/outputs and usage
documentation are always current here without keeping a duplicate in this
repository — but unlike the `docs/` files above, it's rendered as its own
top-level `action.html` page (`ACTION_DOC`/`build_action_page`), reachable
from the main nav's "Action" entry (`pages/includes/header.html`) and from
the "GitHub Action" integration card on the homepage, rather than filed
under `docs/` as if it were reference material rather than a primary
integration. It reuses the same `render_doc`/`raw_github_link` machinery a
`DOCS` entry's remote `content_url` uses: `ACTION_DOC` sets `content_url` to
the raw file and `source_url` to that other repository's own blob URL
(`raw_github_link`'s "View raw source on GitHub" link uses `source_url`
when a doc sets it, instead of assuming the file lives under this
repository's own `docs/`), so the rendered page links back to the
authoritative source. A failed download fails the build rather than
silently publishing stale documentation.

`pages/videos.json` is a simple JSON list of the project's video
walkthroughs — each entry a YouTube `id` and a `title` — rendered by
`pages/build.py`'s `render_videos_list`/`build_videos_page` into
`pages/dist/videos.html` via `pages/videos.template.html`: one embedded
YouTube player per entry, oldest first. Adding a new video means adding
an entry to `pages/videos.json`, not touching `build.py` or the
template. Both `index.template.html` and `docs.template.html` link to it
from their nav bar's "Videos" entry.

`pages/coverage.template.html` renders into `pages/dist/coverage.html`, a
per-module, per-file line coverage breakdown for the version shown in the
site's footer, so visitors can get an impression of how well tested the
project is without digging through CI artifacts themselves. Unlike every
other page above, its content isn't derived from anything checked into the
repository: `build.py`'s `build_coverage_content` renders it from a
directory of downloaded lcov reports passed via `--coverage-dir`, grouping
and formatting them with `.github/scripts/coverage_summary.py`'s own
`MODULES` list and `pct()` helper (loaded by file path via
`load_coverage_summary`, since `.github/scripts` isn't an importable
Python package) so the breakdown can never drift from the module grouping
already used in the release notes and pull request coverage comments;
`parse_lcov_files`/`normalize_source_path` add the per-file granularity
`coverage_summary.py` itself doesn't need, stripping a CI runner's
absolute checkout prefix off each lcov `SF:` path so files display
relative to the repository root. Within each report, files are listed
worst-covered first so weak spots are immediately visible. Omitting
`--coverage-dir` (a local preview build, or no successful CI run found for
the displayed version) renders the page with a "data unavailable"
placeholder instead of failing the build. This doesn't need its own CI
job: the existing GitHub Pages workflow (see below) resolves the same
version shown in the footer, finds that commit's most recent successful
`ci.yml` run the same way `release.yml`'s `release-notes` job does,
downloads its coverage artifacts if one exists, and passes them straight
to `--coverage-dir`.

`pages/imprint.template.html` renders into `pages/dist/imprint.html`, the
legal notice (Impressum) required for a site operated from Germany. Unlike
every other page above, it carries no build-time placeholder for its own
body content at all — `build_imprint_page` just runs it through
`render_shared_components` for the shared header/footer/version chrome,
since the legal text itself never changes at build time. It's linked from
`pages/includes/footer.html` (as "Legal Notice") rather than the main nav,
so every page across the site — not just the homepage — carries a direct
link to it, as German law (§5 TMG) requires.

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
already exist. The `ubuntu-latest` leg also copies the checked-in
`docs/papyrus-lint.default.yaml` (see Configuration above) to
`papyrus-lint.yaml` and attaches it to the release alongside the CLI
binary, rather than generating it by running the freshly built CLI's
`init` subcommand. A separate `editor-plugins` job runs independently,
packages the VS Code extension into a `.vsix` (via `@vscode/vsce`)
and the `SublimeLinter-contrib-papyrus-lint` directory into a `.zip`, and
attaches both to the same release. A final `release-notes` job (after
both `release` and `editor-plugins` succeed) overwrites the release's
title and body — replacing the generic body `tauri-apps/tauri-action`
set on the `release` job — with the tag name as the title; a changelist
of the merged pull requests between the previous and current tag,
resolved per commit via the "list pull requests associated with a
commit" GitHub API and linked with the PR title as text, prefixed with
that pull request's `type: *` label(s) (e.g. `[Feature]`, or
`[Feature, Tests]` when a pull request carries more than one) when it
carries any — a pull request with none is listed with no such prefix;
the current
code coverage (aggregated the same way as CI's coverage-comment job,
via `.github/scripts/coverage_summary.py`, from the lcov artifacts of
the most recent successful `ci.yml` run for the tagged commit); and a
link to the full changelist (`.../compare/<previous-tag>...<tag>`). The
changelist itself is grouped into a section per `component: *` label
(see Pull request labels below) in a fixed order — Sublime Text Plugin,
VS Code Extension, Frontend, Linting, CI, Parsing, Pages, GUI, then CLI —
with a pull request carrying more than one of those labels listed under
every matching component in that order; a pull request whose only
matching label is `component: documentation` is left out of the release
notes entirely, since documentation changes are tracked elsewhere, while
one labeled `component: documentation` alongside other component labels
is still listed under every one of those other components. A pull request
matching none of
the labels above falls into a trailing "Other" section instead of
failing the job. It also builds a plain-text version of the same PR
changelist (titles only, no PR numbers or links) and uploads it as the
`nexus-changelog` artifact for the `nexus-upload` job below; a pull
request is left out of this version if it carries `component:
documentation`, `component: pages`, `component: ci`, `type: tests`,
`type: documentation`, or `type: dependency` — regardless of what else
it's labeled, since none of those describe anything a Nexus downloader
would notice, unlike the
GitHub release notes above where a `component: ci`/`component: pages`
pull request still gets its own section.

A final `nexus-upload` job (after `release`, `editor-plugins`, and
`release-notes` all succeed) publishes the release to the project's
[Nexus Mods page](https://www.nexusmods.com/skyrimspecialedition/mods/189862),
authenticating with the `NEXUSMODS_API_KEY` repo secret, via the
[`Nexus-Mods/upload-action`](https://github.com/Nexus-Mods/upload-action).
It downloads the already-built assets straight off the GitHub release
(rather than rebuilding anything) — the Windows installer (`*setup.exe`),
`PapyrusLinterCLI-windows.exe`, the `docs/papyrus-lint.default.yaml` copy
uploaded as `papyrus-lint.yaml` (zipped locally, since Nexus expects it
as an archive), the SublimeLinter plugin `.zip`, and the VS Code
extension `.vsix` (also zipped, for the same reason) — and uploads each
as a new version of its corresponding Nexus mod file: the two
executables as `main` files, the editor plugins as `optional`, and the
zipped config as `miscellaneous`. The setup.exe upload also sets
`primary_mod_manager_download` and `update_mod_version`, since it's the
mod's primary download and drives the mod-level version shown on the
page; it additionally passes `mod_id` and the downloaded
`nexus-changelog` artifact's content as `changelog`, so that same
upload also posts the version's changelog entry to the mod page (the
action requires `mod_id` whenever `changelog` is set). The Nexus API has
no endpoint to update a mod's page description, so
`docs/nexuspage.bbcode` is not synced by this job and still needs to be
pasted onto the mod page by hand.

An `update-pages` job (after `release`) triggers `pages.yml` (see GitHub
Pages above) via `gh workflow run pages.yml --ref the-one -f version=...`,
passing the tag (`github.ref_name`) as its `version` input; this is the
only thing that ever deploys the site, since `pages.yml` has no `push`
trigger of its own (see GitHub Pages above for why). It dispatches a
separate run pinned to `the-one` rather than invoking
`pages.yml` in-line as a `workflow_call` (which would otherwise seem the
more obvious choice, and once ran that way): a reusable `workflow_call`
runs on the caller's own ref, which for this tag-triggered workflow is
the tag itself rather than a branch, and the `deploy` job's
`github-pages` environment has a deployment branch policy that only
allows `the-one` — so that run always failed with "Branch/tag not
allowed to deploy to github-pages due to environment protection rules"
regardless of the tag's actual content. Dispatching `pages.yml` to run
on `the-one` keeps the ref a branch the environment allows, at the cost
of the dispatched run no longer being nested under this workflow run (it
shows up as its own `GitHub Pages` run) and needing its own `actions:
write` permission to fire the dispatch.

## Merging

Before merging a pull request, make sure its branch is up to date with
`the-one` (the default branch). Merge or rebase `the-one` into the branch
first if it has fallen behind, so CI runs against the current base.

## Pull request labels

Every pull request must be tagged with at least one label naming the
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

The `release-notes` job (see Releases above) groups its changelist by
these labels instead of listing merged pull requests flat, so an
unlabeled (or mislabeled) pull request falls into a trailing "Other"
section of the release notes rather than under its actual component.

Additionally, a pull request may carry one or more `type: *` labels to help
the CI job described above recommend the next semantic version:

- `type: breaking change` — a major-version bump (the first digit)
- `type: feature` — a minor-version bump (the second digit)
- `type: refactoring` — a patch-version bump (the last digit)
- `type: tests` — a patch-version bump (the last digit)
- `type: documentation` — a patch-version bump (the last digit)
- `type: dependency` — a patch-version bump (the last digit), the same as a
  bugfix; used for dependency version updates

Unlike the component labels above, these are optional and purely advisory:
nothing enforces them on a pull request, and omitting one just means that
pull request contributes no recommendation of its own.

Every pull request must also carry at least one `type: ...` label (e.g.
`type: feature`, `type: documentation`) naming the kind of change it
makes. Unlike the component labels above, this set isn't fixed by this
document — add a new `type: ...` label in the repository's label
settings if a pull request doesn't fit an existing one.

CI's `labels` job (see CI below) enforces both requirements on every pull
request and fails before running the rest of CI if either is missing. A
Dependabot pull request satisfies this automatically: the
`dependabot-labels.yml` workflow (triggered by `pull_request_target` on
`opened`/`reopened`, since a `pull_request`-triggered workflow gets a
read-only token for Dependabot's own pull requests) tags every one with
`type: dependency`, plus `component: ci` for a `github-actions` ecosystem
update or the component matching the directory Dependabot updated
otherwise (`/app` → `component: frontend`, `/app/src-tauri` →
`component: gui`, `/app/crates/papyrus-parser` → `component: parsing`,
`/app/crates/papyrus-lints` → `component: linting`) via
[`dependabot/fetch-metadata`](https://github.com/dependabot/fetch-metadata)'s
`package-ecosystem`/`directory` outputs. A directory not in that list (i.e.
not yet one `.github/dependabot.yml` configures) only gets `type:
dependency`, the same as any other pull request that still needs a
`component: ...` label added by hand.

## Current state

The parser (`app/crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence. Each parsed `FunctionDecl` records the name of the
`State` block it was declared in (`None` for a function/event declared
directly on the script, i.e. the "empty state"), so downstream tooling can
tell a state override apart from its base declaration; `papyrus-lint-core`'s
`function_table.rs` carries that same `state` field through into its
cross-script function signatures, and includes a function declared only
inside a state (with no matching empty-state declaration) rather than
silently dropping it. Each parsed integer literal (`Literal::Int`) also
records the `IntFormat` (`Decimal` or `Hexadecimal`) it was written with,
so downstream tooling can tell a hex-written literal apart from a decimal
one without re-scanning the original source text; `papyrus-lints`'
`formid-hex-notation` lint reads the same distinction off the lexer's
`TokenKind::IntLiteral` tokens directly, since it works on tokens rather
than the parsed AST. The parsed `Script` also records the line its
`ScriptName` keyword starts on, and a parsed `Stmt::If` records the
line/column its `Else` keyword starts on (`None` when it has no `Else`
clause at all, distinguishing that from an empty `Else` clause, which both
leave `else_body` empty) — so `papyrus-lints`' `property-sorting` and
`empty-body` lints can read that information straight off the AST instead
of re-lexing the source, on a script that parses cleanly at all;
`empty-body`'s `Else` check still falls back to scanning tokens directly
when the script doesn't parse, since that's the only way it can still run
then.

`app/crates/papyrus-lints` currently implements all rules listed in the
[README's Implemented Lints table](README.md#implemented-lints). Rules inspect
raw source or lexer tokens rather than requiring a successfully parsed AST.
Automatic repair is available for trailing whitespace, comma spacing,
semicolons, indentation, whitespace around member-access dots, spacing
around `!` negation, spacing around logical/comparison operators, and
property sorting (disabled by default; see the README). The desktop app,
standalone CLI, and editor extensions all use the same lint and repair
engine.

`papyrus-lints`' `tags` module publishes a `RuleTags` entry — kind
keyword(s) (e.g. `"style"`, `"performance"`, `"correctness"`,
`"maintainability"`), an `Importance` (`Low`/`Medium`/`High`) rating how
much fixing that rule matters for keeping a codebase maintainable, a
`description` copied verbatim from that rule's row in the README's
[Implemented Lints](README.md#implemented-lints) tables (kept in sync by
hand the same way `docs/nexuspage.bbcode`'s own lint descriptions are —
see "Keeping agent instructions synchronized" below), and an
`auto_fixable()` method derived from `FIXABLE_RULE_IDS` rather than stored
separately, so the two can never drift apart — for every id in
`KNOWN_RULE_IDS`, looked up case-insensitively via `tags::tags_for`. The
desktop app's `list_rule_tags` Tauri command (`app/src-tauri/src/lib.rs`)
exposes the same metadata to the frontend as a JSON-friendly
`RuleTagsInfo` per rule (its `description` is also what the Lint results
tab's "Export for AI" button carries in each `rule_details` entry — see
the README's Export for AI documentation); `app/src/main.ts` fetches it
once at startup
(`loadRuleTags`/`applyRuleTags`), indexes it by rule id, and uses it both
to render each lint finding's kind/importance/auto-fixable badges (see
`buildFindingTagsEl`) and to drive the Lint results tab's filters
(`matchesTagFilters`), alongside its existing severity and filename
filters. "Filter by tag / rule" combines what used to be a separate
flat "Filter by rule" multiselect with the tag kind checkboxes into one
fieldset (`populateRuleFilterGroups`): each kind (Style, Performance,
Correctness, Maintainability) gets its own multiselect listing just the
rules tagged with that kind — a rule tagged with more than one kind
(e.g. `argument-types`, tagged both `performance` and `correctness`)
appears in each of its kinds' own lists, kept in sync with each other
through the single `activeRules` set they all read from/write to
(`syncRuleFilterSelections`) — and the kind's own checkbox is a "select
all"/"select none" toggle for its multiselect (reflecting a partial
selection as indeterminate; `updateTagKindHeaderCheckbox`) rather than an
independent filter dimension of its own. The separate "Show
importance"/"Auto-fixable only" filters are unaffected. A finding whose
rule carries no tag metadata (e.g. a compiler-reported diagnostic; see
`app/src-tauri/src/compile_diagnostics.rs`) always passes those filters
rather than being hidden. The CLI's `--tag <kind>` flag builds on the
same metadata to run only one kind's worth of lints/fixes at a time,
matched case-insensitively against a rule's `kinds` (e.g. `--tag style`):
given without `fix`, it restricts the reported diagnostics to matching
rules; given with `fix`, it also restricts which automatic fixes run, via
`papyrus_lints::repair_filtered_by_tag` (a sibling of `repair_filtered`,
which does the same for a single rule id via `fix --type`, both built
atop a shared private `repair_with` that takes an `applies(rule) -> bool`
predicate). `--tag` can't be combined with `--type`, since the two select
overlapping things (one rule vs. one kind of rule), and an unrecognized
tag is a usage error.

Project configuration is read from an optional `papyrus-lint.yaml` or
`papyrus-lint.yml` in the project root. Both the desktop app and the CLI are
forgiving of an achlist that doesn't live in the project root itself (e.g.
dropped next to a game's `Data` directory while the project lives in a
subfolder): each first tries to find the root from where the achlist's own
resolved scripts sit under a `Scripts/Source` or `Source/Scripts` pair
(`projectDirForAchlist` in `app/src/main.ts`; `find_candidate_pair_root` in
`papyrus-lint-cli`), falling back to the achlist's own parent directory (the
conventional layout) only if none of them do. The CLI resolves a bare `.psc`
given directly the same way, walking up from the file itself and falling
back to two directories above it if no such pair is found at all; the
desktop app's frontend still uses that simpler fixed "two directories up"
rule for a bare `.psc` dropped directly (`projectDirForPscPath`).

Given a directory instead of an `.achlist`/`.psc` path, the CLI
recursively scans it (and every subdirectory beneath it, at any depth)
for `.psc` files instead
(`papyrus_lint_core::script_locator::find_psc_files_recursively`) and
lints every one found — for a project (e.g. Requiem's own layout) with no
`.achlist` at all whose scripts are spread across arbitrarily nested
subfolders rather than a flat `scripts/source`. Its project root is found
the same way as an achlist's: each resolved script is tried against
`find_candidate_pair_root` first, falling back to the scanned directory
itself (rather than an achlist's parent directory, which doesn't apply
here) only if none of them match. The desktop app exposes the same scan
as a `list_psc_files_recursively` Tauri command, used by
`handleDroppedPaths` in `app/src/main.ts` when a single dropped path is
neither an `.achlist` nor a `.psc` file (it errors out for a path that
isn't an existing directory either, which the frontend treats the same as
today's "drop a single .achlist or .psc file" case); `projectDirForDirectory`
mirrors `find_candidate_pair_root`'s fallback using the same
`findCandidatePairRoot` helper `projectDirForAchlist` already uses.
Cross-script resolution across the discovered subfolders works the same
way it does for achlist entries, since both share the code path that adds
each resolved script's parent directory as an additional search root.

By default, the CLI (and the desktop app) resolves cross-script lookups
among an achlist's own entries by treating every listed entry's parent
directory as a generic additional search root — which matters for achlists
whose entries are grouped into arbitrary source directories rather than
either conventional layout, since a script in one listed directory then
still resolves a script type declared in another listed directory. This
also means an unlisted file that happens to sit in one of those
directories resolves too, and `conflicting_script_versions` scans every
such directory in full, which got expensive on a modlist-sized achlist
(hundreds of listed files across as many directories; see
[#311](https://github.com/Idrinth/papyrus-lint/issues/311)). The CLI avoids
repeating that scan once per listed script by building a
`script_locator::ScriptIndex` (`build_script_index`) — a one-time map from
each directory's file names to their paths — up front, then using it both
for cross-script name resolution and to check every script via
`conflicting_script_versions_in_index` instead of
calling `conflicting_script_versions` (which scans afresh on every call)
per script; the desktop app's single-file `lint_psc_file`/`repair_psc_file`
commands still call `conflicting_script_versions` directly, since they
only ever check one script per invocation. Setting the project's
`strict_achlist_scope` to `true` switches the CLI to registering
each listed `.psc` directly with the `FunctionTable`
(`FunctionTable::with_known_scripts` in `papyrus-lint-core`) instead,
scoping resolution strictly to what the achlist actually lists — cross-listed-directory
resolution still works, but nothing unlisted leaks in, and no directory is
scanned at all — with `script_locator::conflicting_script_versions_among`
covering the one case directory scanning otherwise catches for free: two
listed entries sharing a file name. It defaults to `false` so an existing
achlist-based project's resolution/diagnostics don't change underneath it.

The desktop app picks each project's lint configuration explicitly, right
after a drop resolves which project directory is actually in play, rather
than showing/editing whatever configuration happened to be loaded
previously (or the engine's silent defaults) before the user has even said
which project it applies to. `loadProjectConfig` (`app/src/main.ts`) is
what `handleDroppedPaths` calls, in place of calling `useProjectDir`
directly: for a project directory not yet confirmed this session, it locks
the entire Settings tab (`setSettingsLocked`, backed by a `<fieldset
id="settings-fieldset" disabled>` wrapping every Settings tab control, plus
a `#settings-locked-notice` paragraph explaining why) and shows the
`#config-picker` dialog (`promptForConfigSelection`) before doing anything
else. "Continue" (or Escape, or a backdrop click) accepts whatever
`useProjectDir`'s own auto-detection would already do — the project's
existing `papyrus-lint.yaml`/`.yml`, or the engine's silent defaults if it
has none; an inline list of presets (shown only when the project has none
yet — see below) lets the user click one to start from instead; and a text
field lets the user point at a specific configuration file instead. The
Settings tab's own "Configuration
file" input (`configPathOverrideEl`) is set to that typed path (or cleared,
for the other two choices) before `useProjectDir` actually loads and
applies the resulting configuration and the Settings tab is unlocked. A
project directory already confirmed this session (e.g. dropping the same
achlist again) skips the dialog entirely and reuses whatever was picked the
first time. `useProjectDir` itself stays a plain, reusable "load this
already-decided directory's configuration" function with no dialog of its
own, since other call sites — the "Configuration file" input changing,
once the tab is already unlocked — need to reload a project's
configuration without re-asking which one to use. The app doesn't restore
whichever project directory was last open at startup: the Settings tab
simply stays locked (see `setSettingsLocked`) until the user actually
drops something this session. Silently reloading a remembered directory's
configuration on startup would have been pointless anyway — nothing marks
that directory confirmed, so dropping that same project again still goes
through the picker dialog, which then picks (and applies) its
configuration itself, overwriting whatever the silent restore had just
loaded.

Editing the "Configuration file" input directly, once unlocked, still
overrides project-directory discovery entirely the same way it always has:
when set, the app reads/writes lint settings at that exact path instead —
via the `load_lint_config_from_path`/`save_lint_config_to_path` Tauri
commands, which wrap `papyrus_lint_core::config::load_config_from_path`
(also used by the CLI's own `--config <path>`) and
`config::save_config_at_path`. Unlike before, it's no longer remembered in
`localStorage` independent of the loaded project: a value chosen for one
project has no bearing on the next one dropped, since each project's
configuration is picked fresh via the dialog above rather than a page-load
default the user could edit before dropping anything. Leaving it blank
reverts to the normal auto-detection described above. Editing lint
settings while an override is active saves to that file (creating it if it
doesn't exist yet, preserving any other settings — e.g. `compiler_path` —
already stored in it) instead of the current project directory's own
config file; `compiler_path`, `compile_check`, and
`additional_script_roots` themselves are unaffected by this override and
still follow the loaded project directory.

Configuration controls formatting, lint enablement, complexity thresholds,
CLI failure levels, and the compiler path. It also controls whether the
desktop app's `lint_psc_file`/`repair_psc_file` commands additionally run
PapyrusCompiler.exe against a dropped `.psc` as part of linting it
(`compile_check`, off by default), merging in any errors it reports (see
`app/src-tauri/src/compile_diagnostics.rs`) alongside the lint engine's
own; unlike the "Compile"/"Save & Compile" buttons, this always compiles
into a throwaway temporary directory rather than the project's real
output directory. See the [README configuration
reference](README.md#configuration) for the per-key documentation, and
[`docs/papyrus-lint.default.yaml`](docs/papyrus-lint.default.yaml) — the
same file `PapyrusLinterCLI init` writes and the one the README links to
instead of dumping inline — for the complete default file. That file must
stay byte-for-byte identical to `PapyrusLinterCLI init`'s output (built
from `papyrus_lints::Config::default()` and the `FIELD_COMMENTS` table in
`papyrus-lint-core/src/config.rs`, which is what actually generates the
per-key comments): `papyrus-lint-core`'s
`config::tests::default_config_matches_the_checked_in_docs_copy` test
fails CI if they drift, so regenerate it with `PapyrusLinterCLI init`
(and update `FIELD_COMMENTS`/README together) whenever a default or a
field comment changes.

`PapyrusLinterCLI init` also accepts `--preset <strict|standard|careful|name>`
(matched case-insensitively, defaulting to `strict`), which selects the
baseline `config::Preset` (`papyrus-lint-core/src/config.rs`) it generates
`papyrus-lint.yaml` from, in place of the hardcoded default. `strict` is
identical to `papyrus_lints::Config::default()` (and to
`docs/papyrus-lint.default.yaml`), so plain `init` — no `--preset` — is
unaffected by the flag existing at all; `standard` and `careful` are less
noisy. Each built-in preset's own annotated YAML lives under
[`docs/presets/`](docs/presets/) (`papyrus-lint.strict.yaml`,
`.standard.yaml`, `.careful.yaml`) and is compiled into the binary via
`include_str!`, rather than read from disk at runtime.

Any other name is resolved as a user preset instead:
`config::Preset::parse` accepts any non-blank name that isn't one of the
three built-ins as `Preset::Custom(name)` without touching the filesystem
yet, and `Preset::yaml(base_dir)` — called once `init` actually needs the
preset's YAML — looks for a `<name>.yaml`/`.yml` file (matched
case-insensitively via `config::find_user_preset_file`) inside a `presets`
directory (`config::USER_PRESETS_DIR_NAME`) next to `base_dir`
(`config::user_presets_dir`/`user_presets_dir_under`), erroring out if
`base_dir` is unavailable or no such file exists there. This mirrors the
executable-adjacent base config below: both live next to the same
executable, resolved through the same `base_dir`/`executable_dir()` split so
tests can supply a controlled directory instead of depending on the test
binary's own `current_exe()`. `config::list_user_preset_names(dir)` lists
every such file's stem (sorted case-insensitively), for the desktop app's
preset picker below.

The CLI's `preset add <name> <path-to-papyrus-lint.yaml> [--yes]` subcommand
creates a user preset from an existing config file, rather than requiring
one to be placed under the executable-adjacent `presets` directory by hand:
`config::add_user_preset(name, source_path, overwrite)` copies
`source_path`'s contents into that directory as `<name>.yaml` (creating the
directory first if needed), refusing `name` if it's blank or matches a
built-in preset name case-insensitively (`AddPresetError::InvalidName`),
since such a name could never actually be selected — `Preset::parse` always
resolves a built-in first. If a preset named `name` already exists there
(as either `.yaml` or `.yml`), it's left untouched and
`AddPresetError::AlreadyExists` is returned unless `overwrite` is `true`;
the CLI surfaces this as `--yes`, the confirmation an overwrite requires
since the CLI has no interactive prompt, and reuses the existing file's own
extension when overwriting rather than creating a second file alongside it.
Split into a private `add_user_preset_under(base_dir, ...)` the same way as
`initialize_default_config`/`initialize_config_with_base` above, so tests
can supply a controlled directory instead of depending on the test binary's
own `current_exe()`.

`papyrus-lint-cli`'s `doctor <path-to-achlist-or-psc-or-directory>`
subcommand (`run_doctor` in `papyrus-lint-cli/src/lib.rs`) validates a
project's setup — the paths its configuration assumes or names — without
linting any script. It accepts the same `--config`/`--script-root` flags a
plain lint/fix run does, so it reports on exactly the project a matching
run would actually use, and resolves the achlist/`.psc`/directory input
and the project root the same way `run` does
(`find_candidate_pair_root`/`find_psc_project_root`). Each check (the
input path itself existing; every listed `.achlist` entry existing on
disk; the discovered or `--config`-named config file parsing; at least one
of `scripts/source`/`source/scripts` existing under the project root;
each configured `additional_script_roots`/`--script-root` entry resolving
to an existing directory; a configured or auto-detected `compiler_path`
pointing at an existing file) is collected as a `DoctorCheck` — an
`ok`/`warning`/`error` `DoctorStatus` plus a message — rather than
aborting the run on the first problem found, so a single invocation
reports the full picture at once. `compiler_path` being unset and
unauto-detectable is only ever a `warning` when `compile_check` is also
enabled (checked separately, via `resolve_compiler_path`) — otherwise it's
harmless and reported as `ok`, since the CLI itself never needs
`compiler_path` outside that setting. Printed as one `[ok]`/`[warning]`/
`[error] <message>` line per check in the plain-text report, or as a
`DoctorReport` (`project_root`, `checks`, `success`) JSON document with
`--json`; exits `0` if every check passed, `1` if any reported a `warning`
or `error`, or `2` on a usage error, the same convention every other
subcommand follows.

`initialize_default_config` also looks for a `papyrus-lint.yaml`/`.yml`
file next to the running executable (`config::executable_dir`, backed by
`std::env::current_exe()`) and, if one exists, layers it over the selected
preset's own YAML instead of using the preset alone: any key it sets
overrides the preset, any key it omits still falls back to the preset.
This is a recursive per-key merge over `serde_yaml::Value` (`deep_merge`)
rather than serde's own `#[serde(default)]` handling, since which
"default" a key falls back to now depends on the chosen preset at runtime
instead of being fixed at compile time; the merge covers `rules:`'s own
nested keys too, so a base file that only overrides a couple of individual
rules still inherits every other rule from the selected preset. This lets
someone define their own baseline settings once, next to wherever they
keep the CLI or desktop app binary, and reuse it across every project they
run `init` in — layered on top of whichever preset they pick each time —
rather than hand-editing each newly generated file the same way
afterward. The lookup is split into a private `initialize_config_with_base(dir,
base_dir, preset)` so tests can supply a controlled `base_dir` instead of
depending on the test binary's own `current_exe()`; the checked-in
`docs/papyrus-lint.default.yaml` copy is unaffected since CI's test
environment has no such file next to the test binary, and the `strict`
preset (`init`'s own default) reproduces it byte-for-byte.

The desktop app offers the same presets from its own config-picker dialog
above, rather than only through the CLI's `init --preset` flag:
`promptForConfigSelection` fetches every preset via the
`list_config_presets` Tauri command (only when the project has no
`papyrus-lint.yaml`/`.yml` yet, per `load_project_info`'s result — a preset
is pointless to offer once one's already been detected) and renders one
button per preset directly into the dialog's own `#config-picker-preset-list`
(hidden entirely when the list comes back empty), rather than opening a
second, nested dialog for it: the preset list is only ever meaningful as
part of this one choice, so there's nothing else it needs to be its own
dialog for. `list_config_presets` is backed by `papyrus-lint-core`'s
`presets` module (`presets::all()`) — a thin label/description layer over
`config::Preset`/`config::PRESET_NAMES`, the same enum the CLI flag parses,
which appends a `PresetInfo` (id/label both the file's stem, a generic
description) for every name `config::list_user_preset_names` finds under
the executable-adjacent `presets` directory (`config::user_presets_dir`),
after the three built-ins; `presets::all()`/`PresetInfo`'s fields are owned
`String`s rather than `&'static str`, since a user preset's identity is
discovered from a file name at runtime instead of being a compile-time
constant. Clicking one resolves `promptForConfigSelection` with
`{ kind: "preset", preset: id }`, which `loadProjectConfig` then hands to
`apply_config_preset(dir, id)` — resolving the id via `config::Preset::parse`
and handing it to `config::initialize_default_config`, the very function
`init --preset` itself calls — so the desktop app gets the same "refuse to
replace an existing config" guard, executable-adjacent base-config
layering, and user-preset resolution for free.

The Settings tab's own "Save current settings as preset…" button goes the
other direction: `handleSaveConfigAsPresetClick` (`app/src/main.ts`) prompts
for a name, then — if `list_config_presets` already lists a preset under it
(matched case-insensitively; a built-in name is rejected by the backend
outright, since `config::Preset::parse` always resolves those first) —
confirms overwriting it before calling the `save_config_as_preset` Tauri
command with the currently edited `LintConfig`, the name, and whether to
overwrite. That command wraps `papyrus-lint-core`'s
`config::save_user_preset`, which writes just the lint settings (not a
project's own `compiler_path`/`additional_script_roots`/`compile_check`/
`strict_achlist_scope`, which aren't something a reusable preset should
hardcode) as `<name>.yaml` under the same executable-adjacent `presets`
directory the picker above and `list_user_preset_names` read from —
creating that directory first if it doesn't exist yet — so the saved
preset is immediately selectable from the picker, or via the CLI's
`--preset <name>`, without restarting anything.

A "Presets" tab (`#tab-presets`/`#panel-presets`, next to Settings) lets a
user rename, export, or delete their own saved presets afterward, without
touching the filesystem by hand. It's only shown once at least one exists:
`renderPresetManagementTab` (`app/src/main.ts`) filters whatever
`list_config_presets` returns down to the non-built-in ones
(`isCustomPreset`, checking a preset's id against the three built-in names
by hand — the same convention `FIXABLE_RULE_IDS` follows for the lint
engine's own fixable rule ids), hides the tab entirely when that list is
empty, and switches back to the Settings tab if it was the active one and
its last preset just disappeared. `refreshPresetManagementTab` re-fetches
and re-renders it, called once at startup and after every action below
that could change which presets exist (including
`handleSaveConfigAsPresetClick` itself, above). Each listed preset gets
three buttons:

- **Rename** (`handleRenamePresetClick`) prompts for a new name, applying
  the same "cancel on a blank prompt" and "confirm before overwriting an
  already-used name" rules `handleSaveConfigAsPresetClick` does, then
  calls the `rename_user_preset` Tauri command
  (`papyrus_lint_core::config::rename_user_preset`), which finds the
  preset's existing `<name>.yaml`/`.yml` file in the executable-adjacent
  `presets` directory and renames it in place — refusing a blank or
  built-in new name the same way `save_user_preset` does, and preserving
  the file's own extension.
- **Delete** (`handleDeletePresetClick`) confirms, then calls
  `delete_user_preset` (`config::delete_user_preset`), which removes the
  matching file from the `presets` directory.
- **Export** (`handleExportPresetClick`) calls `export_user_preset`
  (`config::read_user_preset_yaml`), which returns the preset's file
  contents verbatim (unlike `initialize_default_config`, it isn't merged
  against an executable-adjacent base config or a project's own settings),
  and downloads it as `<id>.yaml` via the same Blob-and-anchor technique
  `handleExportIssuesClick` uses for the Lint results tab's own export
  button, so it needs no Tauri fs/dialog plugin either. Built-in presets
  can't be renamed, deleted, or exported this way, since none of the three
  backend functions above ever resolve a name matching
  `config::PRESET_NAMES`.

The Settings tab also has a "Reset to preset…" control (a
`#reset-to-preset-select` dropdown, listing every preset the same way the
"Reset" button next to it is populated by `populateResetPresetSelect` —
kept in sync with the Presets tab via `refreshPresetManagementTab`, since
both list the same presets) for undoing a settings change gone wrong,
rather than hand-editing every field back: `handleResetToPresetClick`
confirms overwriting the currently edited settings (since, unlike
`apply_config_preset`, this can discard settings already saved to a
project's real config file, not just a scratch in-memory edit), then
fetches the selected preset's lint settings via the `get_preset_lint_config`
Tauri command and applies them through the exact same
`applyLintConfigToUI`/`handleLintConfigChanged` path any manual field edit
already goes through, so the reset is written wherever settings are
already being saved (the current project directory, or an active
"Configuration file" override) with no separate save codepath of its own.
`get_preset_lint_config` wraps `papyrus-lint-core`'s new
`config::preset_lint_config_default` — a sibling of
`initialize_default_config` sharing its preset-resolution and
executable-adjacent base-config-layering logic (`resolve_preset_project_file`)
but returning just the resolved `papyrus_lints::Config` instead of writing
a brand new project file, and neither refusing an already-existing config
nor touching a project's own `compiler_path`/`additional_script_roots`/
`compile_check`/`strict_achlist_scope` settings, since those aren't
something resetting a project's *lint* settings back to a preset should
touch.

The desktop app's `parse_psc_file` command, both the app's and the CLI's
cross-script lookups (`papyrus-lint-core`'s `function_table.rs`, used to
resolve the "Argument type check"/"Return type check" lints across
scripts), and the desktop app's `lint_psc_file`/`repair_psc_file`/
`repair_psc_finding`/`repair_psc_file_rule` commands and the CLI's own
per-script lint loop (via `ast_cache::ensure_primed`, see below) cache
each parsed `.psc` AST on disk
(`app/crates/papyrus-lint-core/src/ast_cache.rs`), in an `ast-cache`
directory next to the running executable — the desktop app's own binary,
or `PapyrusLinterCLI`'s, whichever process is doing the parsing. A cached
entry is only reused when its stored MD5 of the file's content and the
file's last-modified timestamp still match, and the linter version that
wrote the entry is at or above a `MIN_COMPATIBLE_VERSION` constant
(currently `1.28.0`) rather than an exact match against the running
version — so an ordinary app update doesn't discard an otherwise
still-valid cache, and `MIN_COMPATIBLE_VERSION` only needs bumping when a
release actually changes the cache entry layout or the AST shape it
embeds. Any mismatch, or any I/O/(de)serialization failure reading the
cache, falls back to a fresh parse, so a stale or corrupt cache never
surfaces as a lint error. Since it lives in `papyrus-lint-core`, the same
cache backs the editor extensions too, which invoke `PapyrusLinterCLI` as
a subprocess. The on-disk entry also carries a `tokens` field alongside
`ast` (both `Option`s, so writing one preserves the other's still-valid
cached value via a read-modify-write against the existing entry), for the
lexer's own token stream (`papyrus_parser::tokenize()`'s output), cached
via `get_tokens`/`put_tokens` the same way `get`/`put` cache the AST;
`put_tokens` is also called directly (alongside `put`) wherever
`parse_psc_file` and `function_table.rs`'s cross-script lookups freshly
parse a script, independent of `ast_cache::ensure_primed` below. The
on-disk entry format (the `modified_unix_secs`/`content_md5`/
`linter_version`/`ast`/`tokens` envelope, and the `ast`/`tokens` fields'
own shape) is published as a [JSON
Schema](docs/ast-cache-entry.schema.json) using JSON Schema Draft
2020-12, versioned the same way the cache itself is: it describes
entries whose `linter_version` is at or above `MIN_COMPATIBLE_VERSION`,
so a consuming tool should check a read entry's `linter_version` against
its own known-compatible floor the same way before trusting this schema,
and bump that floor whenever `MIN_COMPATIBLE_VERSION` moves. Update it
alongside any change to `CacheEntry` or to `papyrus_parser::ast::Script`
that bumps `MIN_COMPATIBLE_VERSION`.

Since `papyrus_lints::lint()`/`repair()` parse and tokenize their `source`
argument internally and never see a file path, they can't consult
`ast_cache` directly by themselves. `ast_cache::get`/`get_tokens` close
that gap as a side effect: a disk cache hit for either also primes
`papyrus-parser`'s own in-memory memoization (`papyrus_parser::prime_cache`/
`prime_tokenize_cache`, see below) with the same value, so anything that
parses/tokenizes that exact source text later in the same process --
including a lint/repair pass's own internal `parse()`/`tokenize()` calls --
reuses it instead of redoing the work. `ast_cache::ensure_primed` wraps
both accessors for the "about to lint/repair a script whose disk cache
might already be current" case: a disk cache hit for either primes the
matching in-memory cache as above; a miss for either parses/tokenizes the
source once itself (which populates the in-memory cache the same way a hit
would) and writes a fresh disk entry for next time. It's what the app's
`lint_psc_file`/`repair_psc_file`/`repair_psc_finding`/`repair_psc_file_rule`
commands and the CLI's own per-script lint loop call before linting/
repairing, so relinting an unchanged script -- across separate desktop app
commands or CLI invocations, not just the narrower `parse_psc_file`/
cross-script-lookup cases above -- skips both re-parsing and re-tokenizing
it too.

`papyrus-parser`'s own `parse()` and `tokenize()` entry points
(`app/crates/papyrus-parser/src/cache.rs`) are memoized in-memory against
the most recently seen source string: a single
`papyrus_lints::lint()`/`lint_with_external_arguments()` pass over one
script calls into them dozens of times (each AST-based lint rule calls
`parse()`, each rule that works on raw tokens instead — e.g.
`chain_whitespace`, `exclamation_spacing`, `indentation` — calls
`tokenize()`) with the exact same source text, so a single-slot,
thread-local cache keyed by content equality turns all but the first call
into a clone instead of a re-lex/re-parse. This is deliberately simpler
than the disk-backed `ast_cache`: it never outlives the process (or even
the thread) and so needs no path, mtime, or version bookkeeping, since it
only ever has to remember the one source string a lint/repair pass is
currently working on. `papyrus_parser::prime_cache`/`prime_tokenize_cache`
(`cache.rs`'s `prime`/`prime_tokens`) are the two ways this in-memory
cache is seeded from outside the crate, letting a caller that already has
a validated AST/token stream for that same source text -- namely
`ast_cache::get`/`get_tokens`, above -- insert it directly instead of
leaving the pass's first `parse()`/`tokenize()` call to compute it.

The desktop app's code viewer has a "Preview fixes" button next to its
whole-file "Apply fixes" button, the GUI counterpart of the CLI's
`fix --dry-run`: `app/src/main.ts`'s `handleCodeViewerPreviewFixClick`/
`previewRepairPscFile` call the `preview_repair_psc_file` Tauri command
(`app/src-tauri/src/lib.rs`), which computes the same whole-file repair
(`papyrus_lints::repair`) `repair_psc_file` applies but never writes it to
disk, returning a standard unified diff (empty when nothing would change)
instead. `renderDiffOutput` shows that diff in a `<pre id="code-viewer-
diff-output">` panel beneath the viewer, coloring added/removed/context/
header lines via their own `code-viewer__diff-line--*` class the way a
typical diff viewer does. The diff renderer itself
(`unified_diff`) lives in `papyrus_lint_core::diff` — moved there from
`papyrus-lint-cli`'s own formerly-private `diff` module, which now imports
it from there instead — so the CLI's `fix --dry-run` and the desktop app's
`preview_repair_psc_file` command share the exact same diff output. The
"Preview fixes" button shares "Apply fixes"'s visibility rule (view mode
only, at least one fixable finding remaining) via
`updateCodeViewerFixButtonsVisibility`; the shown preview itself is
cleared again on switching to Edit or once a real "Apply fixes" run
actually changes the file, since a stale preview would no longer be
accurate at that point.

## Keeping agent instructions synchronized

`AGENTS.md` and `CLAUDE.md` must contain the same project guidance. Whenever
one file is updated, make the equivalent update to the other file in the same
change and verify that the two files remain identical.

Whenever the documented lints in `README.md` are updated, make the corresponding
lint update to `docs/nexuspage.bbcode` in the same change. Preserve the Nexus page's
existing style: keep its lint descriptions shorter and more concise than the
README rather than copying the README's longer explanations verbatim. Whenever
the CLI usage examples/options or the documented default configuration in
`README.md` are updated, make the corresponding update to the CLI or
configuration section of `docs/nexuspage.bbcode` in the same change. Other README
changes do not need to be synchronized to the Nexus page.

`pages/index.template.html` (see GitHub Pages above) needs no such manual
sync for the documented lints or CLI usage examples: `pages/build.py`
generates those sections directly from `README.md` on every deploy, so
they can't drift. Its remaining hand-authored prose (the hero pitch, the
"what this is/isn't" cards, the configuration/editor-integrations
blurbs) should still be kept roughly in step with `README.md` by hand
when those parts of the README change meaningfully, the same as
`docs/nexuspage.bbcode`'s own intro prose above.

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
│       │       └── native_globals.rs   # Known native singleton scripts (Game,
│       │                               # Utility, Debug, ...) always called by
│       │                               # literal name, with no .psc in the
│       │                               # project; reads rules/native-globals.yaml
│       │                               # via a build-time-generated array (build.rs)
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
    ├── styles.css              # with content converted straight from README.md,
    ├── fonts/                  # renders every docs/* file into a browsable subpage
    │   ├── cinzel-v26-latin-700.woff2  # (via docs.template.html) linked from a
    │   └── inter-v20-latin-variable.woff2  # Documentation section, renders
    ├── build.py                # videos.json's list of YouTube videos into
    │                            # videos.html (via videos.template.html), and
    │                            # assembles pages/dist/ (git-ignored), copying
    │                            # its assets/ images from resources/ and the
    │                            # app icon rather than committing duplicates of
    │                            # either under pages/, generating a WebP/AVIF
    │                            # sibling of each one rendered as an <img> and
    │                            # rewriting that <img> into a <picture> offering
    │                            # them (see GitHub Pages below), and its fonts/
    │                            # woff2 files as-is so styles.css's @font-face
    │                            # rules self-host Cinzel/Inter instead of
    │                            # pulling them from
    │                            # fonts.googleapis.com/fonts.gstatic.com
    │                            # (avoiding a third-party request on every page
    │                            # load).
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

- **Rules YAML lint job**: runs `yamllint` against every `rules/*.yaml` file
  so malformed rule data cannot be merged.
- **CI scripts and Nexus page BBCode job**: runs the dependency-free Python
  scripts' unit tests with coverage, publishes the text summary, uploads the
  lcov report as the `ci-scripts-coverage` artifact, then checks
  `docs/nexuspage.bbcode` for unknown, mismatched, and unclosed tags.
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
  above over its page(s), writing each page's JSON/HTML report into an
  `app-lighthouse-reports` artifact. A per-page audit failure is logged as
  a workflow warning and fails the job, without stopping the remaining
  pages from being audited. It calls the same
  `.github/scripts/lighthouse_summary.py` with a second `App` argument, so
  its Markdown summary (posted to the job's step summary and as an
  updated-in-place PR comment, same as above) uses its own marker/title
  and never overwrites the GitHub Pages job's comment. Comment posting is
  best-effort (`continue-on-error`) since forked PRs get a read-only
  `GITHUB_TOKEN`.
- **Markdown job**: runs markdownlint-cli2 against every `README.md` in the
  repository, using the root `.markdownlint-cli2.yaml` configuration.
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
  module — CI tooling, crates (the four reusable crates combined), app
  (`src-tauri`), UI (the frontend), the Pages builder, and editor plugins
  (the VS Code extension and the Sublime Text plugin combined) — posting the
  result as a single markdown table, updated in place on subsequent pushes,
  as a PR comment (and to the job's step summary). Comment posting is
  best-effort (`continue-on-error`) since forked PRs get a read-only
  `GITHUB_TOKEN`.

Note: CI runs on pushes to `the-one` (the default branch, not `main`) and
on all pull requests.

## GitHub Pages (`.github/workflows/pages.yml`, `pages/build.py`)

A push to `the-one` that touches `pages/**`, `README.md`, `docs/**`,
`resources/**`, or `app/src-tauri/icons/icon.png` (or the workflow file
itself), a manual `workflow_dispatch` run, or `release.yml`'s
`update-pages` job (see Releases below) invoking it as a `workflow_call`,
builds and deploys a discoverability landing page to GitHub Pages. The
repository's Pages source must be set to "GitHub Actions" (Settings →
Pages) for this workflow to publish successfully.

The page's footer displays the current version via a `<!--VERSION-->`
placeholder that `pages/build.py --version <tag>` fills in the same way
as the lint tables/CLI examples below. The workflow resolves that
version itself before building: it takes the caller-supplied `version`
input if `workflow_call`/`workflow_dispatch` provided one, otherwise
falls back to querying `gh release view` for the repository's latest
release tag; if neither resolves (e.g. no release exists yet), the page
shows "unreleased". This keeps an ordinary content-triggered deploy
showing the actual latest release, while `release.yml`'s `update-pages`
job pins it explicitly to the tag it just built.

`pages/index.template.html` is a plain HTML/CSS page (no frontend
framework or bundler) styled to match the desktop app's frontend
(`app/src/styles.css`): the same Cinzel-headed, light/dark-aware
palette. Rather than hand-duplicating the README's lint tables and CLI
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

Every file in the `docs/` directory (see Project structure above) is also
published as its own browsable subpage, so that reference material isn't
only reachable as raw source on GitHub. `pages/build.py`'s `DOCS` list
names each file, a `slug` for its output filename, and a `kind`
(`markdown`, `json-schema`, or plain text) that picks how it's rendered:
a Markdown file (currently `docs/github-actions-example.md` and
`docs/papyrus-lint-action-readme.md`) is converted to HTML the same way
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
working once rendered. `pages/docs.template.html` is the shared page
shell these subpages (and their `docs/index.html` listing) render into,
carrying its own `<!--DOC_TITLE-->`/`<!--DOC_DESCRIPTION-->`/
`<!--DOC_CONTENT-->` placeholders; `index.template.html`'s own
`<!--DOCS_LIST-->` placeholder is filled with the same titles and a short
hand-written blurb per doc from `DOCS`, linking into `pages/dist/docs/`.
Adding a new file under `docs/` that should be published this way means
adding an entry to `DOCS`, not touching either template.

`docs/papyrus-lint-action-readme.md` is a checked-in copy of the
[`papyrus-lint-action`](https://github.com/idrinth/papyrus-lint-action)
repository's own `README.md`, rather than material written for this
repository, so that its GitHub Action's inputs/outputs and usage
documentation are also reachable as a subpage here. Its `DOCS` entry
sets `source_url` to that other repository's own blob URL for the file
(`raw_github_link`'s "View raw source on GitHub" link uses `source_url`
when a doc sets it, instead of assuming the file lives under this
repository's own `docs/`), so the rendered subpage links back to the
authoritative source rather than to this copy. Whenever
`papyrus-lint-action`'s `README.md` changes, copy the update into
`docs/papyrus-lint-action-readme.md` here too, the same way
`docs/nexuspage.bbcode`'s content is kept in sync by hand (see "Keeping
agent instructions synchronized" below).

`pages/videos.json` is a simple JSON list of the project's video
walkthroughs — each entry a YouTube `id` and a `title` — rendered by
`pages/build.py`'s `render_videos_list`/`build_videos_page` into
`pages/dist/videos.html` via `pages/videos.template.html`: one embedded
YouTube player per entry, oldest first. Adding a new video means adding
an entry to `pages/videos.json`, not touching `build.py` or the
template. Both `index.template.html` and `docs.template.html` link to it
from their nav bar's "Videos" entry.

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
commit" GitHub API and linked with the PR title as text; the current
code coverage (aggregated the same way as CI's coverage-comment job,
via `.github/scripts/coverage_summary.py`, from the lcov artifacts of
the most recent successful `ci.yml` run for the tagged commit); and a
link to the full changelist (`.../compare/<previous-tag>...<tag>`). The
changelist itself is grouped into a section per `component: *` label
(see Pull request labels below) in a fixed order — Sublime Text Plugin,
VS Code Extension, Frontend, CI, Parsing, Pages, GUI, then CLI — with a
pull request carrying more than one of those labels grouped under
whichever comes first in that order; a pull request whose only matching
label is `component: documentation` is left out of the release notes
entirely, since documentation changes are tracked elsewhere, while one
labeled `component: documentation` alongside another component label
still groups under that other label. A pull request matching none of
the labels above falls into a trailing "Other" section instead of
failing the job. It also builds a plain-text version of the same PR
changelist (titles only, no PR numbers or links, and the same
`component: documentation` exclusion applied, but without the
per-component grouping/headings) and uploads it as the `nexus-changelog`
artifact for the `nexus-upload` job below.

A final `nexus-upload` job (after `release`, `editor-plugins`, and
`release-notes` all succeed) publishes the release to the project's
[Nexus Mods page](https://www.nexusmods.com/skyrimspecialedition/mods/189862)
via the Nexus Mods API, authenticating with the `NEXUSMODS_API_KEY` repo
secret. It downloads the already-built assets straight off the GitHub
release (rather than rebuilding anything) — the Windows installer
(`*setup.exe`), `PapyrusLinterCLI-windows.exe`, the
`docs/papyrus-lint.default.yaml` copy uploaded as `papyrus-lint.yaml`
(zipped locally, since Nexus expects it as an archive), the SublimeLinter
plugin `.zip`, and the VS Code extension
`.vsix` — and posts the `nexus-changelog` artifact's content as a
changelog entry for the tag's version (`POST /mods/{id}/changelogs`).
Each of the five files is then uploaded as a new version of its
corresponding Nexus mod file (`POST /uploads`, a `PUT` of the file to the
returned presigned URL with matching `Content-MD5`, `POST
/uploads/{id}/finalise`, then `POST /mod-files/{id}/versions` once the
upload reports `available`): the two executables as `main` files, the
editor plugins as `optional`, and the zipped config as `miscellaneous`.
The setup.exe upload also sets `primary_mod_manager_download` and
`update_mod_version`, since it's the mod's primary download and drives
the mod-level version shown on the page. The Nexus API has no endpoint
to update a mod's page description, so `docs/nexuspage.bbcode` is not synced
by this job and still needs to be pasted onto the mod page by hand.

An `update-pages` job (after `release`) invokes `pages.yml` (see GitHub
Pages above) as a reusable `workflow_call`, passing the tag
(`github.ref_name`) as its `version` input, so the deployed GitHub Pages
site's footer reflects the just-released version immediately rather than
waiting for the next content-triggered deploy.

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
much fixing that rule matters for keeping a codebase maintainable, and an
`auto_fixable()` method derived from `FIXABLE_RULE_IDS` rather than stored
separately, so the two can never drift apart — for every id in
`KNOWN_RULE_IDS`, looked up case-insensitively via `tags::tags_for`. The
desktop app's `list_rule_tags` Tauri command (`app/src-tauri/src/lib.rs`)
exposes the same metadata to the frontend as a JSON-friendly
`RuleTagsInfo` per rule; `app/src/main.ts` fetches it once at startup
(`loadRuleTags`/`applyRuleTags`), indexes it by rule id, and uses it both
to render each lint finding's kind/importance/auto-fixable badges (see
`buildFindingTagsEl`) and to drive the Lint results tab's "Show tags"/
"Show importance"/"Auto-fixable only" filters (`matchesTagFilters`),
alongside its existing severity and filename filters. A finding whose
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

The desktop app's `parse_psc_file` command, and both the app's and the
CLI's cross-script lookups (`papyrus-lint-core`'s `function_table.rs`,
used to resolve the "Argument type check"/"Return type check" lints
across scripts), cache each parsed `.psc` AST on disk
(`app/crates/papyrus-lint-core/src/ast_cache.rs`), in an `ast-cache`
directory next to the running executable — the desktop app's own binary,
or `PapyrusLinterCLI`'s, whichever process is doing the parsing. A cached
entry is only reused when its stored MD5 of the file's content and the
file's last-modified timestamp still match, and the linter version that
wrote the entry is at or above a `MIN_COMPATIBLE_VERSION` constant
(currently `1.16.0`) rather than an exact match against the running
version — so an ordinary app update doesn't discard an otherwise
still-valid cache, and `MIN_COMPATIBLE_VERSION` only needs bumping when a
release actually changes the cache entry layout or the AST shape it
embeds. Any mismatch, or any I/O/(de)serialization failure reading the
cache, falls back to a fresh parse, so a stale or corrupt cache never
surfaces as a lint error. Since it lives in `papyrus-lint-core`, the same
cache backs the editor extensions too, which invoke `PapyrusLinterCLI` as
a subprocess. The on-disk entry format (the `modified_unix_secs`/
`content_md5`/`linter_version`/`ast` envelope, and the `ast` field's own
shape) is published as a [JSON
Schema](docs/ast-cache-entry.schema.json) using JSON Schema Draft
2020-12, versioned the same way the cache itself is: it describes
entries whose `linter_version` is at or above `MIN_COMPATIBLE_VERSION`,
so a consuming tool should check a read entry's `linter_version` against
its own known-compatible floor the same way before trusting this schema,
and bump that floor whenever `MIN_COMPATIBLE_VERSION` moves. Update it
alongside any change to `CacheEntry` or to `papyrus_parser::ast::Script`
that bumps `MIN_COMPATIBLE_VERSION`.

Independent of that disk cache, `papyrus-parser`'s own `parse()` and
`tokenize()` entry points (`app/crates/papyrus-parser/src/cache.rs`) are
memoized in-memory against the most recently seen source string: a single
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
currently working on.

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

<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# CI
 (`.github/workflows/ci.yml`)

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
- **Rust clippy job** (`rust-clippy`): a matrix over `app/src-tauri` and all
  four reusable crates under `app/crates` (the same five crates `rust-test`
  below covers) runs `cargo clippy --all-targets -- -D warnings` against
  each crate's own `Cargo.toml` — not just `app/src-tauri` — since they're
  separate crates rather than workspace members and so aren't checked
  together by a single invocation. Only the `app/src-tauri` leg installs
  Tauri's Linux system dependencies, since the other four crates don't need
  them. Runs in parallel with `rust-fmt` (both only `need` the `labels`
  job); `rust-test` (below) `needs` both.
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


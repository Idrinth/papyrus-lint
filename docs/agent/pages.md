<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# GitHub Pages
 (`.github/workflows/pages.yml`, `pages/build.py`)

`pages/build.py` is the top-level orchestrator (`build()`/`main()`, the
homepage, `videos.html`, `imprint.html`, `sitemap.xml`/`robots.txt`); each
other output kind it assembles lives in its own module so each has a
single reason to change: CSS `@import` inlining in `pages/css.py`,
screenshot/logo/schema asset emission in `pages/site_assets.py`, the
shared page chrome (header/footer/funding links) and per-page
finalization in `pages/site_chrome.py`, the `docs/` subpages and
`action.html` in `pages/docs_pages.py`, `rules.html` in
`pages/rules_page.py`, and `coverage.html` alongside the rest of the
coverage-report domain logic in `pages/coverage_report.py`. Markdown
parsing (`pages/markdown_render.py`), syntax highlighting
(`pages/highlighting.py`), and minification (`pages/minify.py`) were
already their own modules - see below. Each module has its own
`pages/test_*.py`.

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
(`app/src/styles.css`): both import `shared/theme.css` for the Cinzel-headed,
light/dark-aware palette (including the System/Light/Dark theme switch)
so those tokens cannot drift apart. The
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
(`applyTheme`/`loadStoredTheme` in `app/src/main.ts`).
`pages/build.py` inlines `shared/theme.css` into the deployed
`styles.css` so the site still ships a single stylesheet. Rather than
hand-duplicating the README's CLI usage examples into that template (and
having to keep them in sync by hand), it carries a `<!--CLI_EXAMPLES-->`
placeholder comment that `pages/build.py` fills in at build time by
extracting and converting the corresponding Markdown code block straight
out of `README.md`, so that content can never drift out of sync. The
homepage's own "Implemented lints" section carries no per-rule content of
its own at all — it only links to `rules.html` (see `pages/rules_page.py`'s
`render_rules_table`/`build_rules_page` below), which is generated
straight from `shared/rules.json` instead. It also
assembles the page's `assets/` directory (`pages/site_assets.py`) by
copying the screenshots
from `shared/images/` and the app icon from `app/src-tauri/icons/icon.png`,
rather than committing duplicate copies of them under `pages/`. For the
assets actually rendered as `<img>` elements (the header logo and the
five screenshots — not `logo.jpg`, only ever referenced as a raw
`og:image`/`twitter:image` URL, and not the favicon, only ever
referenced via `<link rel="icon">`), it additionally writes a WebP and
an AVIF sibling next to the copied original
(`site_assets.convert_to_modern_formats`,
via `Pillow` — see `pages/requirements-build.txt`), losslessly for the
PNG screenshots (so text/lines stay crisp) and lossy for the already-lossy
JPEG logo; `site_assets.wrap_images_with_modern_sources` then rewrites
every such
`<img>` tag, in every page this builder renders, into a `<picture>`
offering those two smaller formats as preferred `<source>`s ahead of the
original as the final fallback (applied to every page by
`pages/site_chrome.py`'s `finalize_page`). Both the
workflow and a contributor previewing the page locally run it as
`python3 pages/build.py --out pages/dist` (the default `--out`), after
`pip install -r pages/requirements-build.txt` and `python3
.github/scripts/build_rules_json.py` (regenerates the git-ignored
`shared/rules.json` `rules_page.py` reads from `shared/rules/*.json` —
see AGENTS.md hard rule 4); its
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
build time into `pages/site_chrome.py`'s `SITE_URL` constant, which every
page's
`og:url`/`og:image`/`twitter:image` tags are built from (via the
`<!--SITE_URL-->` placeholder each template carries, filled in by
`site_chrome.render_shared_components`) rather than hardcoding the domain a second
time; `build.py` also copies `pages/CNAME` itself into `pages/dist/CNAME`
on every build so GitHub Pages keeps serving it there across each
Actions-based deploy, rather than relying solely on the custom-domain
setting under Settings → Pages. `build.py` also writes a `sitemap.xml`
(`sitemap_urls`/`build_sitemap`, rooted at `SITE_URL`) listing the
homepage, `action.html`, `rules.html`, `videos.html`, `coverage.html`,
`imprint.html`, `docs/index.html`,
and every `DOCS` entry's own subpage — built from the same lists that
generate those pages, so it can't drift out of sync with what's actually
published — and a `robots.txt` (`build_robots_txt`) allowing all crawling
and pointing at that sitemap.

Every file in the `docs/` directory (see Project structure above) is also
published as its own browsable subpage, so that reference material isn't
only reachable as raw source on GitHub. `pages/docs_pages.py`'s `DOCS` list names
each local file or remote `content_url`, a `slug` for its output filename, and a `kind`
(`markdown`, `json-schema`, or plain text) that picks how it's rendered:
a Markdown document is converted to HTML the same way
the CLI examples are (headings, paragraphs, fenced code blocks, and
`render_inline`'s inline formatting - see `pages/markdown_render.py`,
extracted out of `build.py` itself since it's a self-contained concern:
a deliberately small, non-CommonMark subset of Markdown that escapes
everything by default rather than passing raw HTML through, which is what
keeps the papyrus-lint-action README fetched over HTTP below from being
able to inject markup into the built site), with its own top-level heading and
first paragraph read back out as the subpage's title/description rather
than duplicated in `DOCS`; a JSON Schema file renders its
`title`/`description` fields plus the pretty-printed schema itself in a
code block; anything else (`configuration/papyrus-lint.default.yaml`,
`docs/nexuspage.bbcode`) renders as a plain code block under a
hand-written title/description in `DOCS`. A link inside a rendered
Markdown doc to another published doc (matched by filename) resolves to
that doc's own subpage; a `../`-relative link into the rest of the
repository resolves on GitHub instead — both via `docs_pages.resolve_doc_href`,
so `docs/github-actions-example.md`'s existing relative links keep
working once rendered. `pages/docs.template.html` is the page template
these subpages (and their `docs/index.html` listing) render into, carrying
its own `<!--DOC_TITLE-->`/`<!--DOC_DESCRIPTION-->`/`<!--DOC_CONTENT-->`
placeholders. All HTML page templates carry `<!--SITE_HEADER-->` and
`<!--SITE_FOOTER-->` markers which `pages/site_chrome.py`'s
`render_shared_components` fills from
`pages/includes/header.html` and `pages/includes/footer.html`; its
depth-aware `<!--ROOT_PATH-->` replacement keeps links correct from both
the site root and `docs/`, while the footer's `<!--VERSION-->` is filled
from the same build argument everywhere. `index.template.html`'s own
`<!--DOCS_LIST-->` placeholder is filled with the same titles and a short
hand-written blurb per doc from `DOCS`, linking into `pages/dist/docs/`.
Adding a new file under `docs/` that should be published this way means
adding an entry to `docs_pages.DOCS`, not touching either template.

`rules.html` (`pages/rules.template.html`, `pages/rules_page.py`'s
`render_rules_table`/
`render_rules_filter_bar`/`build_rules_page`) is a searchable, filterable
reference of every lint rule, generated straight from `shared/rules.json`'s own
metadata (`id`, `severity`, `tags`, `fixable`, a short `description`, and the
full `definition` prose) rather than from `README.md`'s shorter per-category
tables — so a rule's severity, tags, and full documented behavior are always
one page away without duplicating any of that by hand. It's reachable from
the main nav's "All Rules" entry and from a link in the homepage's own
"Implemented Lints" section. Each row carries `data-severity`/`data-tags`/
`data-fixable`/`data-search` attributes; `pages/rules.js` (copied and
minified into the output directory the same way `pages/downloads.js` is)
reads a search box plus severity/tag/auto-fix checkboxes to show/hide rows
client-side, with every control starting in the "show everything" state so
the page is still a complete, browsable table with JavaScript disabled. A
rule's short description is always shown; its full `definition` sits behind
a native `<details>`/`<summary>` disclosure per row rather than another
control `rules.js` has to wire up itself.

The [`papyrus-lint-action`](https://github.com/idrinth/papyrus-lint-action)
repository's own `README.md` is fetched from its `the-one` branch during
every site build, so that its GitHub Action's inputs/outputs and usage
documentation are always current here without keeping a duplicate in this
repository — but unlike the `docs/` files above, it's rendered as its own
top-level `action.html` page (`pages/docs_pages.py`'s
`ACTION_DOC`/`build_action_page`), reachable
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
repository: `pages/coverage_report.py`'s `build_coverage_page` renders it
from a directory
of downloaded lcov reports passed via `--coverage-dir`, using that same
module's `build_coverage_content` to parse and format
them. That module groups and formats reports with
`.github/scripts/coverage_summary.py`'s own `MODULES` list and `pct()`
helper (loaded by file path via `load_coverage_summary`, since
`.github/scripts` isn't an importable Python package) so the breakdown can
never drift from the module grouping already used in the release notes and
pull request coverage comments; `parse_lcov_files`/`normalize_source_path`
add the per-file granularity `coverage_summary.py` itself doesn't need,
stripping a CI runner's absolute checkout prefix off each lcov `SF:` path
so files display relative to the repository root. Within each report,
files are listed worst-covered first so weak spots are immediately
visible. Omitting `--coverage-dir` (a local preview build, or no
successful CI run found for the displayed version) renders the page with
a "data unavailable" placeholder instead of failing the build. This
doesn't need its own CI job: the existing GitHub Pages workflow (see
below) resolves the same version shown in the footer, finds that commit's
most recent successful `ci.yml` run the same way `release.yml`'s
`release-notes` job does, downloads its coverage artifacts if one exists,
and passes them straight to `--coverage-dir`.

`pages/imprint.template.html` renders into `pages/dist/imprint.html`, the
legal notice (Impressum) required for a site operated from Germany. Unlike
every other page above, it carries no build-time placeholder for its own
body content at all — `build.py`'s `build_imprint_page` just runs it through
`site_chrome.render_shared_components` for the shared header/footer/version chrome,
since the legal text itself never changes at build time. It's linked from
`pages/includes/footer.html` (as "Legal Notice") rather than the main nav,
so every page across the site — not just the homepage — carries a direct
link to it, as German law (§5 TMG) requires.

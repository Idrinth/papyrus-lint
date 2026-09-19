"""Documentation subpages and the GitHub Action's own page for the Pages
builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns). Renders every document listed in DOCS
(including remotely sourced documentation) into its own browsable subpage
(via pages/docs.template.html), published under its own doc_url_prefix()
- docs/ for a doc whose source lives in docs/, or schema//configuration/
for one whose source has moved out of docs/ into its own top-level
directory, so the site never publishes a page under /docs/ for content
that isn't actually in docs/ - and the papyrus-lint-action GitHub Action's
own README (ACTION_DOC) into action.html (via pages/action.template.html),
reachable from the main nav's "Action" entry rather than filed under any
of those as if it were reference material.
"""

from __future__ import annotations

import html
import json
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit, urlunsplit
from urllib.request import Request, urlopen

try:
    from pages.highlighting import highlight_code
    from pages.markdown_render import HEADING_RE, first_paragraph, markdown_to_html, strip_markdown_inline
    from pages.site_chrome import SITE_URL, finalize_page, render_shared_components
except ImportError:  # running as pages/build.py
    from highlighting import highlight_code
    from markdown_render import HEADING_RE, first_paragraph, markdown_to_html, strip_markdown_inline
    from site_chrome import SITE_URL, finalize_page, render_shared_components

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent
DOCS_DIR = ROOT / "docs"
SCHEMA_DIR = ROOT / "schema"
CONFIGURATION_DIR = ROOT / "configuration"

GITHUB_BLOB_BASE = "https://github.com/idrinth/papyrus-lint/blob/the-one"

# Every file published as a browsable subpage, alongside a short hand-written
# blurb shown in the docs list on the homepage and on the docs index page.
# `kind` picks how build.py renders that file's own content. Sourced from
# DOCS_DIR unless an entry overrides `source_dir` (the schemata and the
# default configuration live in their own top-level directories, not docs/).
DOCS = [
    {
        "filename": "project-structure.md",
        "slug": "project-structure",
        "kind": "markdown",
        "blurb": "The canonical repository tree and the responsibilities of its apps, crates, integrations, and tooling.",
    },
    {
        "filename": "papyrus-cli-usage.txt",
        "slug": "papyrus-cli-usage",
        "kind": "bash",
        "blurb": "The full call suite of the Papyrus Lint CLI.",
        "title": "Papyrus Lint CLI Usage",
        "description": "The full call suite of the Papyrus Lint CLI."
    },
    {
        "filename": "examples.md",
        "slug": "examples",
        "kind": "markdown",
        "blurb": "Real bugs Papyrus Lint catches that PapyrusCompiler.exe lets through, beyond the README's example.",
    },
    {
        "filename": "github-actions-example.md",
        "slug": "github-actions-example",
        "kind": "markdown",
        "blurb": "A minimal GitHub Actions workflow that lints a project on every push and pull request.",
    },
    {
        "filename": "configuration.md",
        "slug": "configuration",
        "kind": "markdown",
        "blurb": "The full papyrus-lint.yaml configuration reference: every key, presets, and desktop app settings.",
    },
    {
        "filename": "cli.md",
        "slug": "cli",
        "kind": "markdown",
        "blurb": "The full PapyrusLinterCLI reference: every subcommand and flag, JSON output, and exit codes.",
    },
    {
        "filename": "docker.md",
        "slug": "docker",
        "kind": "markdown",
        "blurb": "Running Papyrus Lint as a container in CI, and verifying release image/binary signatures.",
    },
    {
        "filename": "fixing-findings.md",
        "slug": "fixing-findings",
        "kind": "markdown",
        "blurb": "How the desktop app applies, previews, and exports lint fixes.",
    },
    {
        "filename": "compiling-scripts.md",
        "slug": "compiling-scripts",
        "kind": "markdown",
        "blurb": "How the desktop app compiles a script with PapyrusCompiler.exe and strips build machine info.",
    },
    {
        "filename": "papyrus-lint.default.yaml",
        "source_dir": CONFIGURATION_DIR,
        "repo_dir": "configuration",
        "slug": "papyrus-lint-default-yaml",
        "kind": "yaml",
        "title": "Default configuration (papyrus-lint.yaml)",
        "description": (
            "The full papyrus-lint.yaml written into a project with no configuration file yet, with every key's "
            "default value documented inline. See the configuration reference for what each key does."
        ),
        "blurb": "The full default papyrus-lint.yaml, with every key's default value documented inline.",
    },
    {
        "filename": "papyrus-lint.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "papyrus-lint-schema",
        "kind": "json-schema",
        "blurb": "The JSON Schema for papyrus-lint.yaml / papyrus-lint.yml project configuration files.",
    },
    {
        "filename": "papyrus-lint-report.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "papyrus-lint-report-schema",
        "kind": "json-schema",
        "blurb": "The JSON Schema for the report PapyrusLinterCLI --json emits.",
    },
    {
        "filename": "papyrus-lint-ai-export.v3.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "papyrus-lint-ai-export-v3-schema",
        "kind": "json-schema",
        "blurb": "The current JSON Schema for documents produced by the desktop app's Export for AI feature.",
    },
    {
        "filename": "papyrus-lint-ai-export.v2.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "papyrus-lint-ai-export-v2-schema",
        "kind": "json-schema",
        "blurb": "The frozen v2 JSON Schema for Export for AI documents, superseded by v3 above.",
    },
    {
        "filename": "papyrus-lint-ai-export.v1.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "papyrus-lint-ai-export-v1-schema",
        "kind": "json-schema",
        "blurb": "The frozen v1 JSON Schema for Export for AI documents, superseded by v2 above.",
    },
    {
        "filename": "ast-cache-entry.schema.json",
        "source_dir": SCHEMA_DIR,
        "repo_dir": "schema",
        "slug": "ast-cache-entry-schema",
        "kind": "json-schema",
        "description": (
            "The JSON Schema for one entry in the on-disk ast-cache used to skip re-parsing unchanged scripts, "
            "keyed by the cached script's content hash, modification time, and the linter version that wrote it."
        ),
        "blurb": "The JSON Schema for one entry in the on-disk ast-cache used to skip re-parsing unchanged scripts.",
    },
    {
        "filename": "nexuspage.bbcode",
        "slug": "nexuspage-bbcode",
        "kind": "bbcode",
        "title": "Nexus Mods page description (BBCode source)",
        "description": (
            "The BBCode source used for the project's listing on Nexus Mods. Its lint tables are generated from "
            "shared/rules.json; the rest is kept in sync with the README by hand."
        ),
        "blurb": "The BBCode source behind the project's Nexus Mods page listing.",
    },
]

def doc_url_prefix(doc: dict) -> str:
    """The site's own top-level directory a doc's rendered subpage is
    published under. Mirrors the doc's `repo_dir` (defaulting to "docs")
    so a schema/configuration doc's page is filed under schema/
    /configuration/ rather than docs/, now that its source no longer lives
    there."""
    return doc.get("repo_dir", "docs")


DOC_FILENAME_TO_SLUG = {doc["filename"]: doc["slug"] for doc in DOCS if "filename" in doc}
DOC_FILENAME_TO_PREFIX = {doc["filename"]: doc_url_prefix(doc) for doc in DOCS if "filename" in doc}

# The papyrus-lint-action GitHub Action's own README, rendered as its own
# top-level action.html page (linked from the main nav's "Action" entry)
# rather than filed under docs/ as if it were reference material - see
# build_action_page. Downloaded fresh on every build the same way a DOCS
# entry's own content_url is, so it never drifts from the other
# repository's actual README.
ACTION_DOC = {
    "kind": "markdown",
    "content_url": "https://raw.githubusercontent.com/Idrinth/papyrus-lint-action/the-one/README.md",
    "source_url": "https://github.com/Idrinth/papyrus-lint-action/blob/the-one/README.md",
}


def resolve_doc_href(href: str, from_prefix: str = "docs") -> str:
    """Rewrites a link target found inside a doc's own source so it works
    from a published subpage: a link to another published doc resolves to
    that doc's own subpage (matched by filename alone, so a correct
    repository-relative path like `../schema/papyrus-lint.schema.json`
    still resolves even though that doc's source no longer lives under
    docs/) - same-directory when the two docs share a published prefix
    (`from_prefix`, the current doc's own), `../<prefix>/` otherwise. A
    link into the repository resolves on GitHub. A query string or
    fragment on the original link (e.g. `guide.md#setup`) is preserved
    rather than dropped."""
    parts = urlsplit(href)
    target_filename = Path(parts.path).name
    if not parts.scheme and not parts.netloc and target_filename in DOC_FILENAME_TO_SLUG:
        target_slug = DOC_FILENAME_TO_SLUG[target_filename]
        target_prefix = DOC_FILENAME_TO_PREFIX.get(target_filename, "docs")
        path = f"{target_slug}.html" if target_prefix == from_prefix else f"../{target_prefix}/{target_slug}.html"
        return urlunsplit(("", "", path, parts.query, parts.fragment))
    if href.startswith("../"):
        return f"{GITHUB_BLOB_BASE}/{href[len('../'):]}"
    return href


def raw_github_link(doc: dict) -> str:
    """Link to a local doc on GitHub or a configured external source."""
    repo_dir = doc_url_prefix(doc)
    href = doc["source_url"] if "source_url" in doc else f"{GITHUB_BLOB_BASE}/{repo_dir}/{doc['filename']}"
    return (
        f'<p><a class="doc-raw-link" href="{html.escape(href, quote=True)}">'
        "View raw source on GitHub &rarr;</a></p>"
    )


def load_doc_source(doc: dict) -> str:
    """Load a document from this checkout or its configured remote source.

    Remote documentation is downloaded during every Pages build so the
    published copy follows its owning repository without requiring a synced,
    checked-in duplicate here.
    """
    if content_url := doc.get("content_url"):
        request = Request(content_url, headers={"User-Agent": "papyrus-lint-pages-builder"})
        try:
            with urlopen(request, timeout=30) as response:
                return response.read().decode("utf-8")
        except (HTTPError, URLError, TimeoutError, UnicodeDecodeError) as error:
            raise SystemExit(f"Could not download documentation from {content_url}: {error}") from error
    return (doc.get("source_dir", DOCS_DIR) / doc["filename"]).read_text(encoding="utf-8")


def render_doc(doc: dict) -> tuple[str, str, str]:
    """Render one document into title, description, and subpage HTML."""
    source = load_doc_source(doc)
    kind = doc["kind"]
    if kind == "markdown":
        lines = source.splitlines()
        title_match = HEADING_RE.match(lines[0]) if lines else None
        if title_match and len(title_match.group(1)) == 1:
            title = title_match.group(2)
            body_lines = lines[1:]
        else:
            title = doc["filename"]
            body_lines = lines
        description = strip_markdown_inline(first_paragraph(body_lines))
        current_prefix = doc_url_prefix(doc)
        content_html = markdown_to_html(body_lines, lambda href: resolve_doc_href(href, current_prefix))
    elif kind == "json-schema":
        data = json.loads(source)
        title = data.get("title", doc["filename"])
        # A schema's own "description" is written for JSON Schema consumers and can run
        # much longer than a page tagline should be (see ast-cache-entry.schema.json,
        # whose 800+ character description became this page's Largest Contentful Paint
        # element); prefer a DOCS entry's own short "description" when it sets one.
        description = doc.get("description", data.get("description", ""))
        schema_html = highlight_code(json.dumps(data, indent=2), "json")
        content_html = f'<pre class="code-block language-json" tabindex="0"><code>{schema_html}</code></pre>'
    else:
        title = doc["title"]
        description = doc["description"]
        source_html = highlight_code(source, kind)
        content_html = (
            f'<pre class="code-block language-{html.escape(kind, quote=True)}" tabindex="0">'
            f"<code>{source_html}</code></pre>"
        )
    content_html += raw_github_link(doc)
    return title, description, content_html


def doc_href(doc: dict, current_prefix: str | None) -> str:
    """The href to a doc's own rendered subpage, from a page published
    under current_prefix (None for the site root)."""
    target_prefix = doc_url_prefix(doc)
    if current_prefix is None:
        return f"{target_prefix}/{doc['slug']}.html"
    if target_prefix == current_prefix:
        return f"{doc['slug']}.html"
    return f"../{target_prefix}/{doc['slug']}.html"


def render_docs_list_items(doc_results: dict, current_prefix: str | None) -> str:
    items = []
    for doc in DOCS:
        info = doc_results[doc["slug"]]
        items.append(
            "<li>"
            f'<a href="{doc_href(doc, current_prefix)}">{html.escape(info["title"])}</a>'
            f'<p>{html.escape(doc["blurb"])}</p>'
            "</li>"
        )
    return "\n".join(items)


def build_doc_pages(out_dir: Path, doc_results: dict, version: str = "") -> None:
    """Renders each DOCS entry's own subpage under its `doc_url_prefix` (schema/
    configuration entries are published under schema//configuration/, not
    docs/, mirroring where their source actually lives), plus a unified
    docs/index.html cataloguing all of them regardless of where they live."""
    docs_template = (PAGES_DIR / "docs.template.html").read_text(encoding="utf-8")

    def render_page(title: str, description: str, content_html: str, url: str, current_prefix: str) -> str:
        # The docs index itself only ever lives at docs/index.html, so a page
        # published under `docs` links to it same-directory, any other
        # prefix goes up a level first.
        docs_index_href = "index.html" if current_prefix == "docs" else "../docs/index.html"
        page = docs_template.replace("<!--DOC_TITLE-->", html.escape(title))
        page = page.replace("<!--DOC_DESCRIPTION-->", html.escape(description, quote=True))
        page = page.replace("<!--DOC_URL-->", html.escape(url, quote=True))
        page = page.replace("<!--DOCS_INDEX_URL-->", html.escape(docs_index_href, quote=True))
        return page.replace("<!--DOC_CONTENT-->", content_html)

    for doc in DOCS:
        info = doc_results[doc["slug"]]
        prefix = doc_url_prefix(doc)
        prefix_out_dir = out_dir / prefix
        prefix_out_dir.mkdir(exist_ok=True)
        page = render_page(
            info["title"],
            info["description"],
            info["content_html"],
            f"{SITE_URL}{prefix}/{doc['slug']}.html",
            prefix,
        )
        page = render_shared_components(page, "../", version)
        (prefix_out_dir / f"{doc['slug']}.html").write_text(finalize_page(page), encoding="utf-8")

    docs_out_dir = out_dir / "docs"
    docs_out_dir.mkdir(exist_ok=True)
    index_content = f'<ul class="docs-list">{render_docs_list_items(doc_results, "docs")}</ul>'
    index_page = render_page(
        "Documentation",
        "Project reference material and related documentation, published as browsable pages.",
        index_content,
        f"{SITE_URL}docs/index.html",
        "docs",
    )
    index_page = render_shared_components(index_page, "../", version)
    (docs_out_dir / "index.html").write_text(finalize_page(index_page), encoding="utf-8")


def build_action_page(out_dir: Path, version: str = "") -> None:
    """Renders the papyrus-lint-action GitHub Action's own README (ACTION_DOC
    above) into its own top-level action.html page, reachable from the main
    nav's "Action" entry, instead of a docs/ subpage."""
    title, description, content_html = render_doc(ACTION_DOC)
    template = (PAGES_DIR / "action.template.html").read_text(encoding="utf-8")
    for marker in ("<!--ACTION_TITLE-->", "<!--ACTION_DESCRIPTION-->", "<!--ACTION_CONTENT-->"):
        if marker not in template:
            raise SystemExit(f"action.template.html: missing marker {marker}")
    page = template.replace("<!--ACTION_TITLE-->", html.escape(title))
    page = page.replace("<!--ACTION_DESCRIPTION-->", html.escape(description, quote=True))
    page = page.replace("<!--ACTION_CONTENT-->", content_html)
    page = render_shared_components(page, "", version)
    (out_dir / "action.html").write_text(finalize_page(page), encoding="utf-8")

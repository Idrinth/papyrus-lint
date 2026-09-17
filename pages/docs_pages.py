"""Documentation subpages and the GitHub Action's own page for the Pages
builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change. Renders every
document listed in DOCS (including remotely sourced documentation) into
its own browsable subpage under docs/ (via pages/docs.template.html), and
the papyrus-lint-action GitHub Action's own README (ACTION_DOC) into
action.html (via pages/action.template.html), reachable from the main
nav's "Action" entry rather than filed under docs/ as if it were
reference material.
"""

from __future__ import annotations

import html
import json
from pathlib import Path
from urllib.error import HTTPError, URLError
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

GITHUB_BLOB_BASE = "https://github.com/idrinth/papyrus-lint/blob/the-one"

# Every file in docs/ published as a browsable subpage, alongside a short
# hand-written blurb shown in the docs list on the homepage and on the docs
# index page. `kind` picks how build.py renders that file's own content.
DOCS = [
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
        "filename": "papyrus-lint.default.yaml",
        "slug": "papyrus-lint-default-yaml",
        "kind": "yaml",
        "title": "Default configuration (papyrus-lint.yaml)",
        "description": (
            "The full papyrus-lint.yaml written into a project with no configuration file yet, with every key's "
            "default value documented inline. See the README's configuration reference for what each key does."
        ),
        "blurb": "The full default papyrus-lint.yaml, with every key's default value documented inline.",
    },
    {
        "filename": "papyrus-lint-report.schema.json",
        "slug": "papyrus-lint-report-schema",
        "kind": "json-schema",
        "blurb": "The JSON Schema for the report PapyrusLinterCLI --json emits.",
    },
    {
        "filename": "papyrus-lint-ai-export.v3.schema.json",
        "slug": "papyrus-lint-ai-export-v3-schema",
        "kind": "json-schema",
        "blurb": "The current JSON Schema for documents produced by the desktop app's Export for AI feature.",
    },
    {
        "filename": "papyrus-lint-ai-export.v2.schema.json",
        "slug": "papyrus-lint-ai-export-v2-schema",
        "kind": "json-schema",
        "blurb": "The frozen v2 JSON Schema for Export for AI documents, superseded by v3 above.",
    },
    {
        "filename": "papyrus-lint-ai-export.v1.schema.json",
        "slug": "papyrus-lint-ai-export-v1-schema",
        "kind": "json-schema",
        "blurb": "The frozen v1 JSON Schema for Export for AI documents, superseded by v2 above.",
    },
    {
        "filename": "ast-cache-entry.schema.json",
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
            "docs/rules.json; the rest is kept in sync with the README by hand."
        ),
        "blurb": "The BBCode source behind the project's Nexus Mods page listing.",
    },
]

DOC_FILENAME_TO_SLUG = {doc["filename"]: doc["slug"] for doc in DOCS if "filename" in doc}

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


def resolve_doc_href(href: str) -> str:
    """Rewrites a link target found inside a docs/*.md file so it works from
    a published subpage: a link to another published doc resolves to that
    doc's own subpage, a link into the repository resolves on GitHub."""
    if href in DOC_FILENAME_TO_SLUG:
        return f"{DOC_FILENAME_TO_SLUG[href]}.html"
    if href.startswith("../"):
        return f"{GITHUB_BLOB_BASE}/{href[len('../'):]}"
    return href


def raw_github_link(doc: dict) -> str:
    """Link to a local doc on GitHub or a configured external source."""
    href = doc["source_url"] if "source_url" in doc else f"{GITHUB_BLOB_BASE}/docs/{doc['filename']}"
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
    return (DOCS_DIR / doc["filename"]).read_text(encoding="utf-8")


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
        content_html = markdown_to_html(body_lines, resolve_doc_href)
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


def render_docs_list_items(doc_results: dict, link_prefix: str) -> str:
    items = []
    for doc in DOCS:
        info = doc_results[doc["slug"]]
        items.append(
            "<li>"
            f'<a href="{link_prefix}{doc["slug"]}.html">{html.escape(info["title"])}</a>'
            f'<p>{html.escape(doc["blurb"])}</p>'
            "</li>"
        )
    return "\n".join(items)


def build_doc_pages(out_dir: Path, doc_results: dict, version: str = "") -> None:
    docs_out_dir = out_dir / "docs"
    docs_out_dir.mkdir()
    docs_template = (PAGES_DIR / "docs.template.html").read_text(encoding="utf-8")

    def render_page(title: str, description: str, content_html: str, url: str) -> str:
        page = docs_template.replace("<!--DOC_TITLE-->", html.escape(title))
        page = page.replace("<!--DOC_DESCRIPTION-->", html.escape(description, quote=True))
        page = page.replace("<!--DOC_URL-->", html.escape(url, quote=True))
        return page.replace("<!--DOC_CONTENT-->", content_html)

    for doc in DOCS:
        info = doc_results[doc["slug"]]
        page = render_page(
            info["title"], info["description"], info["content_html"], f"{SITE_URL}docs/{doc['slug']}.html"
        )
        page = render_shared_components(page, "../", version)
        (docs_out_dir / f"{doc['slug']}.html").write_text(finalize_page(page), encoding="utf-8")

    index_content = f'<ul class="docs-list">{render_docs_list_items(doc_results, "")}</ul>'
    index_page = render_page(
        "Documentation",
        "Project reference material and related documentation, published as browsable pages.",
        index_content,
        f"{SITE_URL}docs/index.html",
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

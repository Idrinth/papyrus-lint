#!/usr/bin/env python3
"""Loads a built GitHub Pages site in a real browser and checks it for issues.

`pages/test_build.py` only exercises `build.py`'s own conversion logic
(Markdown-to-HTML, table rendering, ...) against small in-memory fixtures; it
never renders the actual built output, so a typo in a relative link, a
missing anchor target, or a page that throws a console error would slip
through unnoticed. This script instead builds the site (or reuses an
already-built `--dist` directory), serves it over local HTTP, and opens every
page in headless Chromium via Playwright to catch:

- JavaScript console errors or uncaught page errors,
- same-origin resources (stylesheets, images, other pages) that fail to load
  or respond with a 4xx/5xx status, and
- internal links (relative hrefs, including `#fragment` anchors) that point
  at a page or an in-page id that doesn't actually exist.

External links (https://github.com/..., Discord, Nexus Mods, ...) are never
actually fetched: doing so would make this "quick" check slow and flaky
against services this repository doesn't control, so they're faked with a
harmless empty response instead (see `check_site`).

Usage: pages/browser_check.py [--dist DIR]  (default DIR: pages/dist)
"""

from __future__ import annotations

import argparse
import functools
import http.server
import re
import sys
import threading
from dataclasses import dataclass, field
from pathlib import Path
from urllib.parse import urljoin, urlsplit

from playwright.sync_api import sync_playwright

PAGES_DIR = Path(__file__).resolve().parent

HREF_RE = re.compile(r'href="([^"]*)"')


@dataclass
class PageIssues:
    console_errors: list[str] = field(default_factory=list)
    page_errors: list[str] = field(default_factory=list)
    failed_requests: list[str] = field(default_factory=list)


def is_local_href(href: str) -> bool:
    if not href or href.startswith("#"):
        return True
    scheme = urlsplit(href).scheme
    return scheme not in ("http", "https", "mailto", "tel")


class _QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:  # noqa: A002 - matches base signature
        pass


def start_server(directory: Path) -> tuple[http.server.ThreadingHTTPServer, str]:
    handler = functools.partial(_QuietHandler, directory=str(directory))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    port = server.server_address[1]
    return server, f"http://127.0.0.1:{port}"


def check_site(dist: Path) -> list[str]:
    html_files = sorted(p.relative_to(dist).as_posix() for p in dist.rglob("*.html"))
    if not html_files:
        return [f"no .html files found under {dist}"]

    server, base_url = start_server(dist)
    problems: list[str] = []
    ids_by_page: dict[str, set[str]] = {}
    links_by_page: dict[str, list[str]] = {}

    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch()
            page = browser.new_page()
            # Waiting on external hosts (badges, Google Fonts, ...) would
            # make this "quick" check slow and flaky against services this
            # repository doesn't control, and they may not even be reachable
            # in a sandboxed CI runner. Fake a harmless empty response for
            # them instead of aborting the request outright: aborting logs
            # its own "failed to load resource" console error, which would
            # otherwise be indistinguishable from a real one. Local resource
            # errors are still caught via the response listener below.
            page.route(
                re.compile(r"^https?://(?!127\.0\.0\.1)"),
                lambda route: route.fulfill(status=200, body=""),
            )

            for rel_path in html_files:
                issues = PageIssues()

                def on_console(msg, i=issues):
                    if msg.type == "error":
                        i.console_errors.append(msg.text)

                def on_pageerror(exc, i=issues):
                    i.page_errors.append(str(exc))

                def on_response(response, i=issues):
                    if response.url.startswith(base_url) and response.status >= 400:
                        i.failed_requests.append(f"{response.url} -> {response.status}")

                page.on("console", on_console)
                page.on("pageerror", on_pageerror)
                page.on("response", on_response)

                url = f"{base_url}/{rel_path}"
                page.goto(url, wait_until="load", timeout=15000)

                ids_by_page[rel_path] = set(
                    page.eval_on_selector_all("[id]", "els => els.map(e => e.id)")
                )
                links_by_page[rel_path] = HREF_RE.findall(page.content())

                page.remove_listener("console", on_console)
                page.remove_listener("pageerror", on_pageerror)
                page.remove_listener("response", on_response)

                for kind, entries in (
                    ("console error", issues.console_errors),
                    ("page error", issues.page_errors),
                    ("failed resource", issues.failed_requests),
                ):
                    for entry in entries:
                        problems.append(f"{rel_path}: {kind}: {entry}")

            browser.close()
    finally:
        server.shutdown()

    known_files = {p.relative_to(dist).as_posix() for p in dist.rglob("*") if p.is_file()}
    for rel_path, hrefs in links_by_page.items():
        for href in hrefs:
            if not is_local_href(href):
                continue

            path_part, _, fragment = href.partition("#")
            if path_part:
                target_url = urljoin(f"http://x/{rel_path}", path_part)
                target_file = urlsplit(target_url).path.lstrip("/")
            else:
                target_file = rel_path

            if target_file not in known_files:
                problems.append(f"{rel_path}: broken link: '{href}' (no such file '{target_file}')")
                continue

            if fragment and fragment not in ids_by_page.get(target_file, set()):
                problems.append(
                    f"{rel_path}: broken link: '{href}' (no element with id '{fragment}' on '{target_file}')"
                )

    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", default=str(PAGES_DIR / "dist"), help="Built site directory to check")
    args = parser.parse_args()

    dist = Path(args.dist)
    if not dist.is_dir():
        print(f"error: {dist} does not exist; build the site first (pages/build.py --out {dist})", file=sys.stderr)
        return 2

    problems = check_site(dist)
    if problems:
        print(f"Found {len(problems)} issue(s):")
        for problem in problems:
            print(f"  - {problem}")
        return 1

    print("No issues found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

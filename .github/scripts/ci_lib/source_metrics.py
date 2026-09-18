"""Ranks source files by size, public exports, uncovered lines, and LOC.

Extracted out of hall_of_shame.py (which mixed this logic with its CLI
entrypoint) with no behavior change.
"""

from __future__ import annotations

import os
import re
import sys
from collections.abc import Iterable
from pathlib import Path

MARKER = "<!-- hall-of-shame-comment -->"

SKIP_DIR_NAMES = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "__pycache__",
    "coverage",
    "coverage-artifacts",
    "dist",
    "docs",
    "node_modules",
    "playwright-report",
    "target",
    "test-results",
    "venv",
}

SKIP_FILE_NAMES = {
    "Cargo.lock",
    "lcov.info",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
}

SOURCE_SUFFIXES = {
    ".cjs",
    ".css",
    ".cts",
    ".html",
    ".js",
    ".json",
    ".mjs",
    ".mts",
    ".py",
    ".rs",
    ".toml",
    ".ts",
    ".tsx",
    ".yaml",
    ".yml",
}

EXPORT_SUFFIXES = {".cjs", ".cts", ".js", ".mjs", ".mts", ".py", ".rs", ".ts", ".tsx"}

REPO_PREFIXES = (
    "app/",
    ".github/",
    "pages/",
    "vscode-extension/",
    "SublimeLinter-contrib-papyrus-lint/",
    "shared/",
    "rules/",
    "docs/",
)

RUST_EXPORT_RE = re.compile(
    r"^\s*pub(?:\([^)]*\))?\s+"
    r"(?:fn|struct|enum|union|trait|type|const|static|mod|use|impl)\b"
)
JS_EXPORT_RE = re.compile(
    r"^\s*(?:export\s+(?:default\s+)?(?:async\s+)?(?:function\*?|class|const|let|var|enum|type|interface|namespace)\b|"
    r"export\s+(?:default\s+)?(?:\{|\*)|"
    r"export\s+default\b|"
    r"module\.exports\b)"
)
PY_EXPORT_RE = re.compile(r"^(?:async\s+)?(?:def|class)\s+([A-Za-z][A-Za-z0-9_]*)\b")


def format_bytes(size: int) -> str:
    if size < 1024:
        return f"{size} B"
    if size < 1024 * 1024:
        return f"{size / 1024:.1f} KiB"
    return f"{size / (1024 * 1024):.1f} MiB"


def is_skipped_dir(name: str) -> bool:
    if name in SKIP_DIR_NAMES:
        return True
    return name.startswith(".") and name != ".github"


def is_reportable_path(path: str) -> bool:
    parts = Path(path).parts
    return "docs" not in parts and Path(path).suffix.lower() not in {".md", ".markdown"}


def iter_source_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(name for name in dirnames if not is_skipped_dir(name))
        for name in filenames:
            if name in SKIP_FILE_NAMES:
                continue
            path = Path(dirpath, name)
            if path.suffix not in SOURCE_SUFFIXES or not is_reportable_path(relative_path(path, root)):
                continue
            files.append(path)
    files.sort()
    return files


def relative_path(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def count_lines(path: Path) -> int:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return 0
    if not text:
        return 0
    return text.count("\n") + (0 if text.endswith("\n") else 1)


def _code_lines(path: Path) -> Iterable[str]:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith(("//", "#", "/*", "*")):
            continue
        yield line


def count_exports(path: Path) -> int:
    suffix = path.suffix
    if suffix not in EXPORT_SUFFIXES:
        return 0
    count = 0
    if suffix == ".rs":
        for line in _code_lines(path):
            if RUST_EXPORT_RE.search(line):
                count += 1
        return count
    if suffix == ".py":
        for line in _code_lines(path):
            if PY_EXPORT_RE.match(line):
                count += 1
        return count
    for line in _code_lines(path):
        if JS_EXPORT_RE.search(line):
            count += 1
    return count


def collect_file_metrics(root: Path) -> list[tuple[str, int, int, int]]:
    """Returns (relative_path, size_bytes, loc, exports) for every source file."""
    rows: list[tuple[str, int, int, int]] = []
    for path in iter_source_files(root):
        try:
            size = path.stat().st_size
        except OSError:
            continue
        rows.append((relative_path(path, root), size, count_lines(path), count_exports(path)))
    return rows


def normalize_lcov_path(raw: str, root: Path) -> str:
    text = raw.replace("\\", "/")
    try:
        return Path(text).resolve().relative_to(root.resolve()).as_posix()
    except (OSError, ValueError):
        pass
    for prefix in REPO_PREFIXES:
        marker = f"/{prefix}"
        index = text.find(marker)
        if index != -1:
            return text[index + 1 :]
        if text.startswith(prefix):
            return text
    return text.lstrip("./")


def parse_uncovered_lines(lcov_dir: Path, root: Path) -> dict[str, int]:
    """Maps a normalized source path to uncovered executable line count.

    When the same file appears in more than one report, the record with the
    larger LF (or, on a tie, the fewer hits) wins so a file is not double-counted.
    """
    best: dict[str, tuple[int, int]] = {}
    if not lcov_dir.is_dir():
        return {}
    for report in sorted(lcov_dir.rglob("lcov.info")):
        current: str | None = None
        found = hit = 0
        try:
            lines = report.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for line in lines:
            if line.startswith("SF:"):
                current = normalize_lcov_path(line[3:], root)
                if not is_reportable_path(current):
                    current = None
                found = hit = 0
            elif line.startswith("LF:"):
                found = int(line[3:] or "0")
            elif line.startswith("LH:"):
                hit = int(line[3:] or "0")
            elif line.startswith("end_of_record") and current:
                previous = best.get(current)
                if previous is None or found > previous[0] or (found == previous[0] and hit < previous[1]):
                    best[current] = (found, hit)
                current = None
    return {path: max(found - hit, 0) for path, (found, hit) in best.items()}


def top_n(rows: list[tuple[str, int]], n: int) -> list[tuple[str, int]]:
    ranked = [(path, value) for path, value in rows if value > 0]
    ranked.sort(key=lambda item: (-item[1], item[0]))
    return ranked[:n]


def render_list(rows: list[tuple[str, int]], formatter) -> list[str]:
    if not rows:
        return ["_none_"]
    return [f"{index}. `{path}` — {formatter(value)}" for index, (path, value) in enumerate(rows, start=1)]


def notice_line(title: str, rows: list[tuple[str, int]], formatter) -> str:
    message = "none" if not rows else "; ".join(f"{path} ({formatter(value)})" for path, value in rows)
    return f"::notice title={title}::{message}"


def render_report(
    size_rows: list[tuple[str, int]],
    export_rows: list[tuple[str, int]],
    uncovered_rows: list[tuple[str, int]] | None,
    loc_rows: list[tuple[str, int]],
    top: int,
) -> str:
    lines = [
        MARKER,
        "### Hall of shame",
        "",
        f"Top {top} source files by byte size, public exports, uncovered executable lines, and lines of code.",
        "Informational only — this does not fail the job.",
        "",
        "#### Largest by size",
        *render_list(size_rows, format_bytes),
        "",
        "#### Most public exports",
        *render_list(export_rows, str),
        "",
        "#### Most uncovered lines",
    ]
    if uncovered_rows is None:
        lines.append("_no coverage reports_")
    else:
        lines.extend(render_list(uncovered_rows, str))
    lines.extend(
        [
            "",
            "#### Longest files (LOC)",
            *render_list(loc_rows, str),
            "",
            "_Size and LOC cover `.rs` / `.ts` / `.js` / `.py` / `.css` / "
            "`.html` / `.json` / `.yaml` / `.toml` outside documentation, "
            "build, and vendor directories. Markdown files are excluded. "
            "Exports count unrestricted-looking "
            "`pub` items in Rust, `export` / `module.exports` in JS/TS, "
            "and public top-level `def`/`class` names in Python. Uncovered "
            "lines come from the job's downloaded `lcov.info` artifacts._",
        ]
    )
    return "\n".join(lines)


def build_rankings(
    root: Path, lcov_dir: Path | None, top: int
) -> tuple[list[tuple[str, int]], list[tuple[str, int]], list[tuple[str, int]] | None, list[tuple[str, int]]]:
    metrics = collect_file_metrics(root)
    size_rows = top_n([(path, size) for path, size, _, _ in metrics], top)
    export_rows = top_n([(path, exports) for path, _, _, exports in metrics], top)
    loc_rows = top_n([(path, loc) for path, _, loc, _ in metrics], top)
    uncovered_rows = None if lcov_dir is None else top_n(list(parse_uncovered_lines(lcov_dir, root).items()), top)
    return size_rows, export_rows, uncovered_rows, loc_rows


def emit_notices(
    size_rows: list[tuple[str, int]],
    export_rows: list[tuple[str, int]],
    uncovered_rows: list[tuple[str, int]] | None,
    loc_rows: list[tuple[str, int]],
) -> None:
    print(notice_line("Hall of shame — size", size_rows, format_bytes), file=sys.stderr)
    print(notice_line("Hall of shame — exports", export_rows, str), file=sys.stderr)
    if uncovered_rows is None:
        print("::notice title=Hall of shame — uncovered lines::no coverage reports", file=sys.stderr)
    else:
        print(notice_line("Hall of shame — uncovered lines", uncovered_rows, str), file=sys.stderr)
    print(notice_line("Hall of shame — LOC", loc_rows, str), file=sys.stderr)

"""Lint the bundled Skyrim base scripts with each built-in preset and compare
the normalized report against a checked-in snapshot.

The CLI is expected to report findings against vanilla Papyrus — those findings
are not a failure. The job fails only when the report drifts from the validated
snapshot (an unexpected rule change) or the CLI exits 2+ (usage/IO/crash).
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import zipfile
from collections import Counter
from collections.abc import Iterable, Sequence
from pathlib import Path

PRESETS: tuple[str, ...] = ("strict", "standard", "careful")
BASE_SCRIPTS_ZIP = Path("shared/scripts/skyrim-scripts.zip")
SNAPSHOT_DIR = Path("testdata/base-scripts")
PRESET_CONFIG = Path("configuration/presets")


class SnapshotError(RuntimeError):
    """Raised when a snapshot cannot be produced or does not match."""


def preset_config_path(root: Path, preset: str) -> Path:
    return root / PRESET_CONFIG / f"papyrus-lint.{preset}.yaml"


def snapshot_path(root: Path, preset: str) -> Path:
    return root / SNAPSHOT_DIR / f"{preset}.summary.txt"


def extract_base_scripts(archive: Path, destination: Path) -> Path:
    """Unpack ``skyrim-scripts.zip`` into ``destination`` and return that path."""
    if not archive.is_file():
        raise SnapshotError(f"base scripts archive not found: {archive}")
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(destination)
    return destination


def normalize_report(report: dict) -> str:
    """Turn a CLI ``--json`` document into a stable, path-independent listing.

    Files with no diagnostics are omitted from the body so the listing stays
    smaller; ``scripts_checked`` in the header still records the full corpus size.
    """
    scripts_checked = int(report.get("scripts_checked") or 0)
    files = report.get("files") or []
    rows: list[tuple[str, int, int, str, str, str]] = []
    for entry in files:
        path = _normalize_path(entry.get("path") or "")
        for diagnostic in entry.get("diagnostics") or []:
            rows.append(
                (
                    path,
                    int(diagnostic.get("line") or 0),
                    int(diagnostic.get("column") or 0),
                    str(diagnostic.get("level") or ""),
                    str(diagnostic.get("rule") or ""),
                    _one_line(diagnostic.get("message") or ""),
                )
            )
    rows.sort()
    header = [
        "# papyrus-lint base-scripts snapshot",
        f"# scripts_checked: {scripts_checked}",
        f"# files_with_diagnostics: {sum(1 for _ in _unique_paths(rows))}",
        f"# total_diagnostics: {len(rows)}",
    ]
    body = [f"{path}\t{line}\t{column}\t{level}\t{rule}\t{message}" for path, line, column, level, rule, message in rows]
    return "\n".join([*header, *body]) + "\n"


def summarize_snapshot(preset: str, canonical: str) -> str:
    """Compact, reviewable form of ``canonical`` checked into git.

    The SHA-256 covers the full listing, so a message-only change still
    mismatches. Per-rule counts make the delta readable in a pull request.
    """
    scripts_checked = 0
    files_with_diagnostics = 0
    total_diagnostics = 0
    counts: Counter[str] = Counter()
    for line in canonical.splitlines():
        if line.startswith("# scripts_checked:"):
            scripts_checked = int(line.split(":", 1)[1])
        elif line.startswith("# files_with_diagnostics:"):
            files_with_diagnostics = int(line.split(":", 1)[1])
        elif line.startswith("# total_diagnostics:"):
            total_diagnostics = int(line.split(":", 1)[1])
        elif line.startswith("#") or not line.strip():
            continue
        else:
            parts = line.split("\t")
            rule = parts[4] if len(parts) > 4 else ""
            counts[rule] += 1
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    lines = [
        f"# preset: {preset}",
        "# papyrus-lint base-scripts snapshot",
        f"# scripts_checked: {scripts_checked}",
        f"# files_with_diagnostics: {files_with_diagnostics}",
        f"# total_diagnostics: {total_diagnostics}",
        f"# sha256: {digest}",
    ]
    for rule, count in sorted(counts.items()):
        lines.append(f"{rule}\t{count}")
    return "\n".join(lines) + "\n"


def _unique_paths(rows: Sequence[tuple[str, int, int, str, str, str]]) -> Iterable[tuple[str, int, int, str, str, str]]:
    seen: set[str] = set()
    for row in rows:
        if row[0] in seen:
            continue
        seen.add(row[0])
        yield row


def _normalize_path(path: str) -> str:
    return path.replace("\\", "/").lstrip("./")


def _one_line(message: str) -> str:
    return " ".join(message.split())


def run_cli(
    cli: Path,
    project: Path,
    config: Path,
    output: Path,
    extra_args: Sequence[str] | None = None,
) -> dict:
    """Run PapyrusLinterCLI and return its JSON report.

    Exit status 0 (clean) and 1 (findings) are both success for this corpus.
    """
    if not cli.exists():
        raise SnapshotError(f"CLI binary not found: {cli}")
    if not config.is_file():
        raise SnapshotError(f"preset config not found: {config}")
    output.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(cli),
        "--json",
        "--short-paths",
        "--color",
        "never",
        "--config",
        str(config),
        "--output",
        str(output),
        str(project),
    ]
    if extra_args:
        command[1:1] = list(extra_args)
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode >= 2:
        stderr = completed.stderr.strip() or completed.stdout.strip() or "(no output)"
        raise SnapshotError(f"CLI exited {completed.returncode}: {stderr}")
    try:
        return json.loads(output.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SnapshotError(f"failed to read CLI JSON report at {output}: {exc}") from exc


def render_snapshot(
    root: Path,
    cli: Path,
    preset: str,
    work_dir: Path,
    extra_args: Sequence[str] | None = None,
) -> str:
    if preset not in PRESETS:
        raise SnapshotError(f"unknown preset {preset!r}; expected one of {', '.join(PRESETS)}")
    extracted = extract_base_scripts(root / BASE_SCRIPTS_ZIP, work_dir / "scripts")
    report = run_cli(
        cli,
        extracted,
        preset_config_path(root, preset),
        work_dir / f"{preset}.json",
        extra_args,
    )
    canonical = f"# preset: {preset}\n{normalize_report(report)}"
    work_dir.mkdir(parents=True, exist_ok=True)
    (work_dir / f"{preset}.full.txt").write_text(canonical, encoding="utf-8")
    return summarize_snapshot(preset, canonical)


def compare_snapshot(actual: str, expected_path: Path) -> str | None:
    """Return a unified diff when ``actual`` does not match the file at ``expected_path``."""
    if not expected_path.is_file():
        return f"missing validated snapshot: {expected_path}"
    expected = expected_path.read_text(encoding="utf-8")
    if actual == expected:
        return None
    import difflib

    diff = "".join(
        difflib.unified_diff(
            expected.splitlines(keepends=True),
            actual.splitlines(keepends=True),
            fromfile=str(expected_path),
            tofile="actual",
            n=3,
        )
    )
    return diff or "snapshots differ (no textual diff produced)"


def write_snapshot(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")

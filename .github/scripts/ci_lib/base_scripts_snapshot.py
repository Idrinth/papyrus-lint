"""Run the CLI against the bundled base scripts and compare its text output."""

from __future__ import annotations

import subprocess
import zipfile
from collections import Counter
from collections.abc import Sequence
from pathlib import Path

PRESETS: tuple[str, ...] = ("strict", "standard", "careful")
GAMES: tuple[str, ...] = ("skyrim", "fallout", "starfield")
SKYRIM_BASE_SCRIPTS_ZIP = Path("shared/scripts/skyrim-scripts.zip")
SKYRIM_EXTENDER_SCRIPTS_ZIP = Path("shared/scripts/skyrim-extender-scripts.zip")
FIXTURE_DIR = Path("fixtures")
PRESET_CONFIG = Path("configuration/presets")


class SnapshotError(RuntimeError):
    """Raised when CLI output cannot be produced."""


def fixture_path(root: Path, preset: str, game: str, extender: bool) -> Path:
    if extender:
        return root / FIXTURE_DIR / f"{game}-extender-{preset}.txt"
    return root / FIXTURE_DIR / f"{game}-base-{preset}.txt"


def extract_base_scripts(archive: Path, destination: Path) -> Path:
    """Unpack ``skyrim-scripts.zip`` into ``destination`` and return that path."""
    if not archive.is_file():
        raise SnapshotError(f"base scripts archive not found: {archive}")
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(destination)
    return destination


def run_cli(
    cli: Path,
    project: Path,
    preset: str,
    output: Path,
    extra_args: Sequence[str] | None = None,
) -> str:
    """Run PapyrusLinterCLI in text mode and return the report.

    Exit status 0 (clean) and 1 (findings) are both successful executions.
    """
    if not cli.exists():
        raise SnapshotError(f"CLI binary not found: {cli}")
    config = Path(project / "papyrus-lint.yaml")
    if config.exists():
        config.unlink()
    command = [
        str(cli),
        "init",
        "--preset",
        preset
    ]
    subprocess.run(command, check=False, capture_output=False, text=True, cwd=project)
    output.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(cli),
        "--format",
        "plain",
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
        return output.read_text(encoding="utf-8")
    except OSError as exc:
        raise SnapshotError(f"failed to read CLI text report at {output}: {exc}") from exc


def render_output(
    root: Path,
    cli: Path,
    preset: str,
    work_dir: Path,
    game: str,
    extender: bool,
    extra_args: Sequence[str] | None = None,
) -> str:
    if preset not in PRESETS:
        raise SnapshotError(f"unknown preset {preset!r}; expected one of {', '.join(PRESETS)}")
    if game != "skyrim":
        raise SnapshotError(f"unknown game {game}; expected one of skyrim")
    path = root / SKYRIM_BASE_SCRIPTS_ZIP
    if extender:
        path = root / SKYRIM_EXTENDER_SCRIPTS_ZIP
    extracted = extract_base_scripts(path, work_dir / "scripts")
    return run_cli(
        cli,
        extracted,
        preset,
        work_dir / f"{preset}.txt",
        extra_args,
    )


def compare_output(actual: str, expected_path: Path) -> str | None:
    """Return added/removed lines, ignoring their order, or ``None`` on a match."""
    if not expected_path.is_file():
        return f"missing baseline: {expected_path}"

    expected_lines = Counter(expected_path.read_text(encoding="utf-8").splitlines())
    actual_lines = Counter(actual.splitlines())
    removed = expected_lines - actual_lines
    added = actual_lines - expected_lines
    if not removed and not added:
        return None

    diff = [f"--- {expected_path}", "+++ actual"]
    diff.extend(f"- {line}" for line in sorted(removed.elements()))
    diff.extend(f"+ {line}" for line in sorted(added.elements()))
    return "\n".join(diff) + "\n"


def write_fixture(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")

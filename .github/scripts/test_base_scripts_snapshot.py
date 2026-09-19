#!/usr/bin/env python3
"""Unit tests for the base-scripts snapshot helper."""

from __future__ import annotations

import io
import json
import stat
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

from ci_lib import base_scripts_snapshot as snap

from base_scripts_snapshot import main as entry_main


def _write_fake_cli(directory: Path, report: dict) -> Path:
    payload = json.dumps(report)
    script = directory / "fake-cli"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        f"REPORT = {payload!r}\n"
        "output = None\n"
        "args = sys.argv[1:]\n"
        "for index, arg in enumerate(args):\n"
        "    if arg == '--output' and index + 1 < len(args):\n"
        "        output = args[index + 1]\n"
        "        break\n"
        "if output is None:\n"
        "    sys.stderr.write('missing --output')\n"
        "    sys.exit(2)\n"
        "from pathlib import Path\n"
        "Path(output).write_text(REPORT, encoding='utf-8')\n"
        "sys.exit(1)\n",
        encoding="utf-8",
    )
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    return script


def _write_repo(directory: Path) -> Path:
    root = directory / "repo"
    (root / "shared" / "scripts").mkdir(parents=True)
    (root / "configuration" / "presets").mkdir(parents=True)
    (root / "testdata" / "base-scripts").mkdir(parents=True)
    archive = root / "shared" / "scripts" / "skyrim-scripts.zip"
    with zipfile.ZipFile(archive, "w") as bundle:
        bundle.writestr("Source/Scripts/Actor.psc", "ScriptName Actor\n")
    for preset in snap.PRESETS:
        (root / "configuration" / "presets" / f"papyrus-lint.{preset}.yaml").write_text(
            f"# {preset}\n",
            encoding="utf-8",
        )
    return root


class NormalizeReportTests(unittest.TestCase):
    def test_sorts_and_drops_empty_files(self) -> None:
        report = {
            "scripts_checked": 3,
            "files": [
                {
                    "path": "Source/Scripts/Zebra.psc",
                    "diagnostics": [
                        {
                            "line": 2,
                            "column": 4,
                            "level": "warning",
                            "rule": "trailing-whitespace",
                            "message": "  trailing   space ",
                        }
                    ],
                },
                {"path": "Source/Scripts/Clean.psc", "diagnostics": []},
                {
                    "path": r"Source\\Scripts\\Actor.psc",
                    "diagnostics": [
                        {
                            "line": 1,
                            "column": 1,
                            "level": "info",
                            "rule": "unused-import",
                            "message": "unused",
                        }
                    ],
                },
            ],
        }
        text = snap.normalize_report(report)
        self.assertIn("# scripts_checked: 3", text)
        self.assertIn("# files_with_diagnostics: 2", text)
        self.assertIn("# total_diagnostics: 2", text)
        summary = snap.summarize_snapshot("strict", f"# preset: strict\n{text}")
        self.assertIn("# total_diagnostics: 2", summary)
        self.assertIn("trailing-whitespace\t1", summary)
        self.assertIn("unused-import\t1", summary)
        self.assertNotIn("Clean.psc", text)
        lines = [line for line in text.splitlines() if not line.startswith("#")]
        self.assertEqual(
            lines,
            [
                "Source/Scripts/Actor.psc\t1\t1\tinfo\tunused-import\tunused",
                "Source/Scripts/Zebra.psc\t2\t4\twarning\ttrailing-whitespace\ttrailing space",
            ],
        )


class CompareSnapshotTests(unittest.TestCase):
    def test_reports_missing_and_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "strict.summary.txt"
            self.assertIn("missing validated snapshot", snap.compare_snapshot("x\n", path))
            snap.write_snapshot(path, "expected\n")
            self.assertIsNone(snap.compare_snapshot("expected\n", path))
            diff = snap.compare_snapshot("actual\n", path)
            assert diff is not None
            self.assertIn("-expected", diff)
            self.assertIn("+actual", diff)


class RenderAndMainTests(unittest.TestCase):
    def test_render_snapshot_and_update_round_trip(self) -> None:
        report = {
            "scripts_checked": 1,
            "files": [
                {
                    "path": "Source/Scripts/Actor.psc",
                    "diagnostics": [
                        {
                            "line": 1,
                            "column": 1,
                            "level": "warning",
                            "rule": "trailing-whitespace",
                            "message": "trailing",
                        }
                    ],
                }
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            cli = _write_fake_cli(base, report)
            work = base / "work"
            actual = snap.render_snapshot(root, cli, "strict", work)
            self.assertTrue(actual.startswith("# preset: strict\n"))
            self.assertIn("trailing-whitespace\t1", actual)
            self.assertIn("# sha256:", actual)
            self.assertIn("# total_diagnostics: 1", actual)

            snap.write_snapshot(snap.snapshot_path(root, "strict"), actual)
            stdout = io.StringIO()
            stderr = io.StringIO()
            with mock.patch("sys.stdout", stdout), mock.patch("sys.stderr", stderr):
                status = entry_main(
                    [
                        "--cli",
                        str(cli),
                        "--root",
                        str(root),
                        "--preset",
                        "strict",
                        "--work-dir",
                        str(base / "work2"),
                    ]
                )
            self.assertEqual(0, status)
            self.assertIn("ok (strict)", stdout.getvalue())

    def test_main_update_writes_snapshot(self) -> None:
        report = {
            "scripts_checked": 1,
            "files": [{"path": "Source/Scripts/Actor.psc", "diagnostics": []}],
        }
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            cli = _write_fake_cli(base, report)
            stdout = io.StringIO()
            with mock.patch("sys.stdout", stdout):
                status = entry_main(
                    [
                        "--cli",
                        str(cli),
                        "--root",
                        str(root),
                        "--preset",
                        "careful",
                        "--update",
                        "--work-dir",
                        str(base / "work"),
                    ]
                )
            self.assertEqual(0, status)
            written = (root / "testdata" / "base-scripts" / "careful.summary.txt").read_text(encoding="utf-8")
            self.assertIn("# preset: careful", written)
            self.assertIn("# total_diagnostics: 0", written)

    def test_unknown_preset_and_cli_crash(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            with self.assertRaises(snap.SnapshotError):
                snap.render_snapshot(root, base / "missing", "nope", base / "work")
            crashing = base / "crash"
            crashing.write_text("#!/bin/sh\necho boom >&2\nexit 2\n", encoding="utf-8")
            crashing.chmod(crashing.stat().st_mode | stat.S_IEXEC)
            with self.assertRaises(snap.SnapshotError) as ctx:
                snap.render_snapshot(root, crashing, "strict", base / "work")
            self.assertIn("exited 2", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()

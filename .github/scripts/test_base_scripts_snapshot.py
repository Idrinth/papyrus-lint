#!/usr/bin/env python3
"""Unit tests for the base-scripts output comparison helper."""

from __future__ import annotations

import io
import stat
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

from base_scripts_snapshot import main as entry_main
from ci_lib import base_scripts_snapshot as snap


def _write_fake_cli(directory: Path, output: str, exit_code: int = 1) -> Path:
    script = directory / "fake-cli"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        f"OUTPUT = {output!r}\n"
        f"EXIT_CODE = {exit_code}\n"
        "args = sys.argv[1:]\n"
        "assert '--format' in args and args[args.index('--format') + 1] == 'plain'\n"
        "assert '--json' not in args\n"
        "output_path = args[args.index('--output') + 1]\n"
        "from pathlib import Path\n"
        "Path(output_path).write_text(OUTPUT, encoding='utf-8')\n"
        "sys.exit(EXIT_CODE)\n",
        encoding="utf-8",
    )
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    return script


def _write_repo(directory: Path) -> Path:
    root = directory / "repo"
    (root / "shared" / "scripts").mkdir(parents=True)
    (root / "configuration" / "presets").mkdir(parents=True)
    for name in ("skyrim-scripts.zip", "skyrim-extender-scripts.zip"):
        archive = root / "shared" / "scripts" / name
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("Source/Scripts/Actor.psc", "ScriptName Actor\n")
    for preset in snap.PRESETS:
        (root / "configuration" / "presets" / f"papyrus-lint.{preset}.yaml").write_text(
            f"# {preset}\n", encoding="utf-8"
        )
    return root


class CompareOutputTests(unittest.TestCase):
    def test_ignores_line_order_and_preserves_duplicate_counts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "strict.txt"
            self.assertIn("missing baseline", snap.compare_output("x\n", path))
            snap.write_fixture(path, "first\nsecond\nfirst\n")
            self.assertIsNone(snap.compare_output("second\nfirst\nfirst\n", path))

            diff = snap.compare_output("second\nthird\nfirst\n", path)
            assert diff is not None
            self.assertIn("- first", diff)
            self.assertIn("+ third", diff)
            self.assertNotIn("- second", diff)

    def test_applies_sibling_extra_delta(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "careful.txt"
            snap.write_fixture(path, "old-summary\nkept\n")
            path.with_name("careful.txt.extra").write_text(
                "- old-summary\n+ new-summary\n+ extra\n",
                encoding="utf-8",
            )
            self.assertIsNone(snap.compare_output("kept\nnew-summary\nextra\n", path))

            diff = snap.compare_output("kept\nold-summary\n", path)
            assert diff is not None
            self.assertIn("- extra", diff)
            self.assertIn("- new-summary", diff)
            self.assertIn("+ old-summary", diff)


class RenderAndMainTests(unittest.TestCase):
    def test_render_output_runs_cli_in_plain_text_mode(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            cli = _write_fake_cli(base, "diagnostic\nsummary\n")
            actual = snap.render_output(root, cli, "strict", base / "work", "skyrim", True)
            self.assertEqual("diagnostic\nsummary\n", actual)

    def test_main_returns_one_and_prints_added_and_removed_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            cli = _write_fake_cli(base, "kept\nadded\n")
            snap.write_fixture(snap.fixture_path(root, "strict", "skyrim", True), "removed\nkept\n")
            stderr = io.StringIO()
            with mock.patch("sys.stderr", stderr):
                status = entry_main(
                    [
                        "--cli",
                        str(cli),
                        "--root",
                        str(root),
                        "--preset",
                        "strict",
                        "--game",
                        "skyrim",
                        "--extender",
                        "--work-dir",
                        str(base / "work"),
                    ]
                )
            self.assertEqual(1, status)
            self.assertIn("- removed", stderr.getvalue())
            self.assertIn("+ added", stderr.getvalue())

    def test_main_update_writes_fixture_and_matching_run_succeeds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            cli = _write_fake_cli(base, "second\nfirst\n")
            common_args = [
                "--cli",
                str(cli),
                "--root",
                str(root),
                "--preset",
                "careful",
                "--game",
                "skyrim",
                "--work-dir",
                str(base / "work"),
            ]
            self.assertEqual(0, entry_main([*common_args, "--update"]))
            self.assertEqual("second\nfirst\n", snap.fixture_path(root, "careful", "skyrim", False).read_text())

            cli = _write_fake_cli(base, "first\nsecond\n")
            self.assertEqual(0, entry_main(common_args))

    def test_unknown_preset_and_cli_crash(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = _write_repo(base)
            with self.assertRaises(snap.SnapshotError):
                snap.render_output(root, base / "missing", "nope", base / "work", "hello", True)
            crashing = _write_fake_cli(base, "", exit_code=2)
            with self.assertRaises(snap.SnapshotError) as ctx:
                snap.render_output(root, crashing, "strict", base / "work", "skyrim", False)
            self.assertIn("exited 2", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()

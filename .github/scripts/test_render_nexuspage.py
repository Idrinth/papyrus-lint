#!/usr/bin/env python3
"""Unit tests for the Nexus page rendering CLI entrypoint."""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("render_nexuspage.py")
SPEC = importlib.util.spec_from_file_location("render_nexuspage", SCRIPT)
assert SPEC and SPEC.loader
render_nexuspage = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = render_nexuspage
SPEC.loader.exec_module(render_nexuspage)


class MainTests(unittest.TestCase):
    def test_main_renders_template_to_output_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            template = root / "template.bbcode"
            output = root / "output.bbcode"
            template.write_text(
                "<COVERED_LINES>/<TOTAL_LINES> (<COVERAGE_PERCENTAGE>) <VERSION>", encoding="utf-8"
            )
            with (
                mock.patch.object(
                    sys,
                    "argv",
                    ["render_nexuspage.py", str(template), str(root), str(output), "v1.2.3"],
                ),
                mock.patch.object(render_nexuspage, "coverage_totals", return_value=(9, 10)),
            ):
                render_nexuspage.main()

            self.assertEqual("9/10 (90.0) v1.2.3", output.read_text(encoding="utf-8"))

    def test_main_aggregates_reports_and_renders_without_mocking_helpers(self) -> None:
        modules = [
            ("First", [("one", "one/lcov.info")]),
            ("Second", [("two", "two/lcov.info")]),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for report, contents in (
                ("one/lcov.info", "LF:4\nLH:3\n"),
                ("two/lcov.info", "LF:6\nLH:2\n"),
            ):
                Path(root, report).parent.mkdir()
                Path(root, report).write_text(contents, encoding="utf-8")
            template = root / "template.bbcode"
            output = root / "output.bbcode"
            template.write_text(
                "<COVERED_LINES>/<TOTAL_LINES> (<COVERAGE_PERCENTAGE>%) <VERSION>",
                encoding="utf-8",
            )

            with (
                mock.patch("ci_lib.nexuspage_render.MODULES", modules),
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "render_nexuspage.py",
                        str(template),
                        str(root),
                        str(output),
                        "v3.4.5",
                    ],
                ),
            ):
                render_nexuspage.main()

            self.assertEqual(
                "5/10 (50.0%) v3.4.5", output.read_text(encoding="utf-8")
            )

    def test_main_rejects_invalid_argument_count(self) -> None:
        for arguments in (
            ["render_nexuspage.py"],
            ["render_nexuspage.py", "template", "artifacts", "output", "version", "extra"],
        ):
            with (
                self.subTest(arguments=arguments),
                mock.patch.object(sys, "argv", arguments),
                self.assertRaisesRegex(SystemExit, "usage: render_nexuspage.py"),
            ):
                render_nexuspage.main()


if __name__ == "__main__":
    unittest.main()

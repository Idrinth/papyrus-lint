#!/usr/bin/env python3
"""Unit tests for the Lighthouse report summary CLI entrypoint."""

import contextlib
import importlib.util
import io
import sys
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("lighthouse_summary.py")
SPEC = importlib.util.spec_from_file_location("lighthouse_summary", SCRIPT)
assert SPEC and SPEC.loader
lighthouse_summary = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = lighthouse_summary
SPEC.loader.exec_module(lighthouse_summary)


class MainTests(unittest.TestCase):
    def test_main_uses_the_default_marker_when_no_label_is_given(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["lighthouse_summary.py", "does-not-exist"]),
            contextlib.redirect_stdout(output),
        ):
            lighthouse_summary.main()

        self.assertIn(lighthouse_summary.MARKER, output.getvalue())
        self.assertIn("### Lighthouse report", output.getvalue())

    def test_main_builds_a_labelled_marker_and_title_when_a_label_is_given(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["lighthouse_summary.py", "does-not-exist", "App"]),
            contextlib.redirect_stdout(output),
        ):
            lighthouse_summary.main()

        rendered = output.getvalue()
        self.assertIn("<!-- lighthouse-summary-comment-App -->", rendered)
        self.assertIn("### App Lighthouse report", rendered)
        self.assertNotIn(lighthouse_summary.MARKER, rendered)


if __name__ == "__main__":
    unittest.main()

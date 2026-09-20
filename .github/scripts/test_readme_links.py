"""Unit tests for baking shared/links.yaml into the editor plugin READMEs."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from ci_lib.links import load_links
from ci_lib.readme_links import README_PATHS, render_readme_links, write_readme_links

SCRIPT = Path(__file__).with_name("write_readme_links.py")
SPEC = importlib.util.spec_from_file_location("write_readme_links", SCRIPT)
assert SPEC and SPEC.loader
write_readme_links_cli = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = write_readme_links_cli
SPEC.loader.exec_module(write_readme_links_cli)


class RenderReadmeLinksTests(unittest.TestCase):
    def test_fills_the_contact_marker_from_the_shared_links_file(self) -> None:
        rendered = render_readme_links("## Contact\n\n<!--CONTACT-LINKS-->\n")

        contact_links = load_links()
        for link in contact_links:
            if "contact" in link.tags:
                self.assertIn(f"- [{link.label}]({link.url})", rendered)
        self.assertNotIn("CONTACT-LINKS", rendered)

    def test_is_a_noop_without_a_marker(self) -> None:
        self.assertEqual("unchanged", render_readme_links("unchanged"))


class WriteReadmeLinksTests(unittest.TestCase):
    def test_writes_rendered_content_to_each_path(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            readme_a = directory / "a" / "README.md"
            readme_b = directory / "b" / "README.md"
            readme_a.parent.mkdir()
            readme_b.parent.mkdir()
            readme_a.write_text("## Contact\n\n<!--CONTACT-LINKS-->\n", encoding="utf-8")
            readme_b.write_text("## Contact\n\n<!--CONTACT-LINKS-->\n", encoding="utf-8")

            write_readme_links([readme_a, readme_b])

            for readme in (readme_a, readme_b):
                content = readme.read_text(encoding="utf-8")
                self.assertNotIn("CONTACT-LINKS", content)
                self.assertIn("- [Discord](https://discord.gg/idrinth)", content)

    def test_default_paths_point_at_both_editor_plugin_readmes(self) -> None:
        self.assertEqual(
            {path.name for path in README_PATHS},
            {"README.md"},
        )
        self.assertEqual(
            {path.parent.name for path in README_PATHS},
            {"vscode-extension", "SublimeLinter-contrib-papyrus-lint"},
        )
        for path in README_PATHS:
            self.assertTrue(path.is_file(), f"missing {path}")


class MainTests(unittest.TestCase):
    def test_main_writes_the_requested_readmes(self) -> None:
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as raw:
            readme = Path(raw) / "README.md"
            readme.write_text("## Contact\n\n<!--CONTACT-LINKS-->\n", encoding="utf-8")

            with (
                mock.patch.object(sys, "argv", ["write_readme_links.py", str(readme)]),
                contextlib.redirect_stdout(output),
            ):
                self.assertEqual(write_readme_links_cli.main(), 0)

            content = readme.read_text(encoding="utf-8")
        self.assertNotIn("CONTACT-LINKS", content)
        self.assertIn(f"Wrote contact links to {readme}.\n", output.getvalue())

    def test_main_defaults_to_both_editor_plugin_readmes(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["write_readme_links.py"]),
            mock.patch.object(write_readme_links_cli, "write_readme_links") as write,
            contextlib.redirect_stdout(output),
        ):
            self.assertEqual(write_readme_links_cli.main(), 0)

        write.assert_called_once_with(list(README_PATHS))


if __name__ == "__main__":
    unittest.main()

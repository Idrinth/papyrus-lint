#!/usr/bin/env python3
"""Unit tests for the hall-of-shame CI script."""

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path


def load_script(name: str):
    path = Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


hall_of_shame = load_script("hall_of_shame")


class HallOfShameTests(unittest.TestCase):
    def test_format_bytes_picks_unit_by_magnitude(self) -> None:
        self.assertEqual("512 B", hall_of_shame.format_bytes(512))
        self.assertEqual("1.5 KiB", hall_of_shame.format_bytes(1536))
        self.assertEqual("1.0 MiB", hall_of_shame.format_bytes(1024 * 1024))

    def test_count_lines_handles_missing_trailing_newline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "file.rs")
            path.write_text("a\nb\nc", encoding="utf-8")
            self.assertEqual(3, hall_of_shame.count_lines(path))
            path.write_text("a\nb\nc\n", encoding="utf-8")
            self.assertEqual(3, hall_of_shame.count_lines(path))
            path.write_text("", encoding="utf-8")
            self.assertEqual(0, hall_of_shame.count_lines(path))

    def test_count_exports_for_rust_js_and_python(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rust = Path(directory, "lib.rs")
            rust.write_text(
                "\n".join(
                    [
                        "pub fn visible() {}",
                        "pub(crate) fn crate_visible() {}",
                        "// pub fn commented() {}",
                        "fn private() {}",
                        "pub struct Thing;",
                        "pub use crate::other::Name;",
                    ]
                ),
                encoding="utf-8",
            )
            typescript = Path(directory, "index.ts")
            typescript.write_text(
                "\n".join(
                    [
                        "export function one() {}",
                        "export const two = 2;",
                        "export { three };",
                        "export default class Four {}",
                        "const notExported = 1;",
                        "// export const commented = 1;",
                    ]
                ),
                encoding="utf-8",
            )
            python = Path(directory, "mod.py")
            python.write_text(
                "\n".join(
                    [
                        "def public():",
                        "    pass",
                        "class Visible:",
                        "    pass",
                        "def _private():",
                        "    pass",
                        "async def also_public():",
                        "    pass",
                    ]
                ),
                encoding="utf-8",
            )

            self.assertEqual(4, hall_of_shame.count_exports(rust))
            self.assertEqual(4, hall_of_shame.count_exports(typescript))
            self.assertEqual(3, hall_of_shame.count_exports(python))

    def test_iter_source_files_skips_docs_markdown_vendor_and_lockfiles(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app").mkdir()
            (root / "app" / "main.rs").write_text("pub fn a() {}\n", encoding="utf-8")
            (root / "node_modules").mkdir()
            (root / "node_modules" / "lib.js").write_text("export const x = 1;\n", encoding="utf-8")
            artifacts = root / "coverage-artifacts" / "frontend-coverage"
            artifacts.mkdir(parents=True)
            (artifacts / "index.html").write_text("<html>" + ("x" * 5000) + "</html>\n", encoding="utf-8")
            (artifacts / "lcov.info").write_text("SF:app/src/main.ts\nLF:8\nLH:1\nend_of_record\n", encoding="utf-8")
            (root / ".github").mkdir()
            (root / ".github" / "scripts").mkdir()
            (root / ".github" / "scripts" / "tool.py").write_text("def run():\n    pass\n", encoding="utf-8")
            (root / "package-lock.json").write_text("{}\n", encoding="utf-8")
            (root / "README.md").write_text("# hi\n", encoding="utf-8")
            (root / "notes.markdown").write_text("# notes\n", encoding="utf-8")
            (root / "docs").mkdir()
            (root / "docs" / "example.py").write_text("def documented():\n    pass\n", encoding="utf-8")

            found = {hall_of_shame.relative_path(path, root) for path in hall_of_shame.iter_source_files(root)}
            self.assertEqual({"app/main.rs", ".github/scripts/tool.py"}, found)

    def test_iter_source_files_skips_hidden_directories_except_github(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".hidden").mkdir()
            (root / ".hidden" / "secret.py").write_text("def secret():\n    pass\n", encoding="utf-8")
            (root / ".github").mkdir()
            (root / ".github" / "tool.py").write_text("def tool():\n    pass\n", encoding="utf-8")

            found = [hall_of_shame.relative_path(path, root) for path in hall_of_shame.iter_source_files(root)]
            self.assertEqual([".github/tool.py"], found)

    def test_count_exports_ignores_unsupported_files_and_private_python_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            markdown = Path(directory, "README.md")
            markdown.write_text("def looks_like_python():\n", encoding="utf-8")
            python = Path(directory, "module.py")
            python.write_text("def _private():\n    pass\nclass _Private:\n    pass\n", encoding="utf-8")

            self.assertEqual(0, hall_of_shame.count_exports(markdown))
            self.assertEqual(0, hall_of_shame.count_exports(python))

    def test_normalize_lcov_path_handles_windows_separators_and_repo_prefixes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            self.assertEqual(
                "app/src/main.ts",
                hall_of_shame.normalize_lcov_path(r"C:\runner\work\repo\app\src\main.ts", root),
            )
            self.assertEqual("unrelated/file.py", hall_of_shame.normalize_lcov_path("./unrelated/file.py", root))

    def test_parse_uncovered_lines_normalizes_and_deduplicates(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reports = root / "coverage-artifacts"
            first = reports / "rust-coverage-core"
            second = reports / "frontend-coverage"
            first.mkdir(parents=True)
            second.mkdir(parents=True)
            (first / "lcov.info").write_text(
                "\n".join(
                    [
                        "SF:/tmp/somewhere/app/crates/core/src/lib.rs",
                        "LF:10",
                        "LH:4",
                        "end_of_record",
                        "SF:/tmp/somewhere/app/crates/core/src/lib.rs",
                        "LF:10",
                        "LH:2",
                        "end_of_record",
                        "SF:/tmp/somewhere/docs/example.py",
                        "LF:50",
                        "LH:0",
                        "end_of_record",
                        "SF:/tmp/somewhere/README.md",
                        "LF:100",
                        "LH:0",
                        "end_of_record",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (second / "lcov.info").write_text(
                "SF:app/src/main.ts\nLF:8\nLH:7\nend_of_record\n",
                encoding="utf-8",
            )

            uncovered = hall_of_shame.parse_uncovered_lines(reports, root)
            self.assertEqual(
                {
                    "app/crates/core/src/lib.rs": 8,
                    "app/src/main.ts": 1,
                },
                uncovered,
            )

    def test_top_n_orders_by_value_then_path_and_drops_zeros(self) -> None:
        rows = [("b.rs", 4), ("a.rs", 4), ("c.rs", 0), ("d.rs", 9)]
        self.assertEqual([("d.rs", 9), ("a.rs", 4), ("b.rs", 4)], hall_of_shame.top_n(rows, 3))

    def test_render_report_uses_empty_placeholders_for_empty_rankings(self) -> None:
        report = hall_of_shame.render_report([], [], [], [], 5)

        self.assertIn("Top 5 source files", report)
        self.assertEqual(4, report.count("_none_"))
        self.assertNotIn("_no coverage reports_", report)

    def test_emit_notices_distinguishes_empty_coverage_from_missing_reports(self) -> None:
        empty_coverage = io.StringIO()
        with contextlib.redirect_stderr(empty_coverage):
            hall_of_shame.emit_notices([], [], [], [])
        self.assertIn("Hall of shame — uncovered lines::none", empty_coverage.getvalue())

        missing_coverage = io.StringIO()
        with contextlib.redirect_stderr(missing_coverage):
            hall_of_shame.emit_notices([], [], None, [])
        self.assertIn("Hall of shame — uncovered lines::no coverage reports", missing_coverage.getvalue())

    def test_main_writes_markdown_and_notice_annotations(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app").mkdir()
            big = root / "app" / "big.rs"
            big.write_text("pub fn one() {}\n" * 20 + "pub struct Two;\n" * 5, encoding="utf-8")
            small = root / "app" / "small.rs"
            small.write_text("fn hidden() {}\n", encoding="utf-8")
            reports = root / "cov"
            reports.mkdir()
            (reports / "lcov.info").write_text(
                f"SF:{big.as_posix()}\nLF:25\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                hall_of_shame.main(["--root", str(root), "--lcov-dir", str(reports), "--top", "3"])

            output = stdout.getvalue()
            notices = stderr.getvalue()
            self.assertIn(hall_of_shame.MARKER, output)
            self.assertIn("### Hall of shame", output)
            self.assertIn("`app/big.rs`", output)
            self.assertIn("::notice title=Hall of shame — size::", notices)
            self.assertIn("::notice title=Hall of shame — exports::", notices)
            self.assertIn("::notice title=Hall of shame — uncovered lines::", notices)
            self.assertIn("::notice title=Hall of shame — LOC::", notices)
            self.assertIn("app/big.rs", notices)

    def test_main_without_lcov_dir_notes_missing_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "only.py").write_text("def public():\n    return 1\n", encoding="utf-8")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                hall_of_shame.main(["--root", str(root), "--no-notices"])

            self.assertIn("_no coverage reports_", stdout.getvalue())
            self.assertEqual("", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()

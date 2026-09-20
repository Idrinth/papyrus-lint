"""Unit tests for the CLI/GUI release hash generation logic."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from ci_lib.cli_release_hashes import (
    ASSETS,
    GUI_ASSETS,
    collect_hashes,
    render_py,
    render_ts,
    sha256_file,
    write_hashes,
)

SCRIPT = Path(__file__).with_name("write_cli_hashes.py")
SPEC = importlib.util.spec_from_file_location("write_cli_hashes", SCRIPT)
assert SPEC and SPEC.loader
write_cli_hashes = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = write_cli_hashes
SPEC.loader.exec_module(write_cli_hashes)


class WriteCliHashesTests(unittest.TestCase):
    def test_hashes_each_release_asset(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            expected = {}
            for asset in ASSETS:
                body = f"{asset} bytes".encode()
                (directory / asset).write_bytes(body)
                expected[asset] = [hashlib.sha256(body).hexdigest()]

            self.assertEqual(collect_hashes(directory), expected)
            self.assertEqual(sha256_file(directory / ASSETS[0]), expected[ASSETS[0]][0])

    def test_appends_gui_hashes_when_those_binaries_are_present(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            expected = {}
            for asset in ASSETS:
                body = f"{asset} bytes".encode()
                (directory / asset).write_bytes(body)
                gui = GUI_ASSETS[asset]
                gui_body = f"{gui} bytes".encode()
                (directory / gui).write_bytes(gui_body)
                expected[asset] = [
                    hashlib.sha256(body).hexdigest(),
                    hashlib.sha256(gui_body).hexdigest(),
                ]

            self.assertEqual(collect_hashes(directory), expected)

    def test_rejects_a_directory_missing_any_cli_asset(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            (directory / ASSETS[0]).write_bytes(b"linux")
            (directory / ASSETS[1]).write_bytes(b"macos")

            with self.assertRaisesRegex(FileNotFoundError, "PapyrusLinterCLI-windows.exe"):
                collect_hashes(directory)

    def test_writes_typescript_and_python_hash_modules(self):
        hashes = {asset: [f"{index:064x}"] for index, asset in enumerate(ASSETS, start=1)}
        with tempfile.TemporaryDirectory() as raw:
            ts_path = Path(raw) / "cliHashes.ts"
            py_path = Path(raw) / "cli_hashes.py"
            write_hashes(hashes, ts_path, py_path)

            ts = ts_path.read_text(encoding="utf-8")
            py = py_path.read_text(encoding="utf-8")
            self.assertEqual(ts, render_ts(hashes))
            self.assertEqual(py, render_py(hashes))
            for asset, digests in hashes.items():
                self.assertIn(f"'{asset}': ['{digests[0]}']", ts)
                self.assertIn(f"'{asset}': ['{digests[0]}']", py)

    def test_rendered_modules_list_cli_and_gui_digests(self):
        hashes = {
            asset: [f"{index:064x}", f"{index + 10:064x}"]
            for index, asset in enumerate(ASSETS, start=1)
        }
        ts = render_ts(hashes)
        py = render_py(hashes)
        for asset, digests in hashes.items():
            expected = f"'{asset}': ['{digests[0]}', '{digests[1]}']"
            self.assertIn(expected, ts)
            self.assertIn(expected, py)


class MainTests(unittest.TestCase):
    def test_main_collects_and_writes_hashes_to_requested_paths(self) -> None:
        hashes = {asset: [f"{index:064x}"] for index, asset in enumerate(ASSETS, start=1)}
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            asset_dir = root / "assets"
            ts_path = root / "cliHashes.ts"
            py_path = root / "cli_hashes.py"
            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "write_cli_hashes.py",
                        str(asset_dir),
                        "--ts-path",
                        str(ts_path),
                        "--py-path",
                        str(py_path),
                    ],
                ),
                mock.patch.object(write_cli_hashes, "collect_hashes", return_value=hashes) as collect,
                mock.patch.object(write_cli_hashes, "write_hashes") as write,
                contextlib.redirect_stdout(output),
            ):
                self.assertEqual(write_cli_hashes.main(), 0)

        collect.assert_called_once_with(asset_dir)
        write.assert_called_once_with(hashes, ts_path, py_path)
        self.assertEqual(
            output.getvalue(),
            f"Wrote CLI/GUI SHA-256 digests to {ts_path} and {py_path}.\n",
        )


if __name__ == "__main__":
    unittest.main()

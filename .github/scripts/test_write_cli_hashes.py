"""Tests for baking PapyrusLinterCLI SHA-256 digests into the editor plugins."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from write_cli_hashes import ASSETS, collect_hashes, render_py, render_ts, sha256_file, write_hashes


class WriteCliHashesTests(unittest.TestCase):
    def test_hashes_each_release_asset(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            expected = {}
            for asset in ASSETS:
                body = f"{asset} bytes".encode()
                (directory / asset).write_bytes(body)
                expected[asset] = hashlib.sha256(body).hexdigest()

            self.assertEqual(collect_hashes(directory), expected)
            self.assertEqual(sha256_file(directory / ASSETS[0]), expected[ASSETS[0]])

    def test_rejects_a_directory_missing_any_cli_asset(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            (directory / ASSETS[0]).write_bytes(b"linux")
            (directory / ASSETS[1]).write_bytes(b"macos")

            with self.assertRaisesRegex(FileNotFoundError, "PapyrusLinterCLI-windows.exe"):
                collect_hashes(directory)

    def test_writes_typescript_and_python_hash_modules(self):
        hashes = {asset: f"{index:064x}" for index, asset in enumerate(ASSETS, start=1)}
        with tempfile.TemporaryDirectory() as raw:
            ts_path = Path(raw) / "cliHashes.ts"
            py_path = Path(raw) / "cli_hashes.py"
            write_hashes(hashes, ts_path, py_path)

            ts = ts_path.read_text(encoding="utf-8")
            py = py_path.read_text(encoding="utf-8")
            self.assertEqual(ts, render_ts(hashes))
            self.assertEqual(py, render_py(hashes))
            for asset, digest in hashes.items():
                self.assertIn(f"'{asset}': '{digest}'", ts)
                self.assertIn(f"'{asset}': '{digest}'", py)


if __name__ == "__main__":
    unittest.main()

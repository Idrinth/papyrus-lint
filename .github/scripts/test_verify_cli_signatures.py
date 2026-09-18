"""Tests for verifying PapyrusLinterCLI Sigstore bundles before baking hashes."""

from __future__ import annotations

import hashlib
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from verify_cli_signatures import (
    VerificationError,
    bundle_name,
    main,
    require_signed_assets,
    verify_signed_cli_assets,
)
from write_cli_hashes import ASSETS


class _FakeRunner:
    def __init__(self, fail_on=None, missing_binary=False):
        self.fail_on = fail_on
        self.missing_binary = missing_binary
        self.calls = []

    def __call__(self, command, check=True, capture_output=True, text=True):
        self.calls.append(list(command))
        if self.missing_binary:
            raise OSError("No such file or directory: 'cosign'")
        if self.fail_on and self.fail_on in command[-1]:
            raise subprocess.CalledProcessError(1, command, stderr="signature mismatch")
        return subprocess.CompletedProcess(command, 0, stdout="verified", stderr="")


def _write_signed_assets(directory: Path) -> dict[str, bytes]:
    bodies = {}
    for asset in ASSETS:
        body = f"{asset} bytes".encode()
        (directory / asset).write_bytes(body)
        (directory / bundle_name(asset)).write_text("{}", encoding="utf-8")
        bodies[asset] = body
    return bodies


class VerifyCliSignaturesTests(unittest.TestCase):
    def test_require_signed_assets_lists_missing_binaries_and_bundles(self):
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            (directory / ASSETS[0]).write_bytes(b"linux")
            (directory / bundle_name(ASSETS[0])).write_text("{}", encoding="utf-8")

            with self.assertRaisesRegex(FileNotFoundError, "PapyrusLinterCLI-macos"):
                require_signed_assets(directory)
            with self.assertRaisesRegex(FileNotFoundError, "PapyrusLinterCLI-windows.exe.sigstore.json"):
                require_signed_assets(directory)

    def test_verify_signed_cli_assets_invokes_cosign_for_each_bundle(self):
        runner = _FakeRunner()
        identity = "https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v1.2.3"
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            _write_signed_assets(directory)

            verify_signed_cli_assets(directory, identity=identity, runner=runner)

        self.assertEqual(len(runner.calls), len(ASSETS))
        for asset, command in zip(ASSETS, runner.calls, strict=True):
            self.assertEqual(command[0], "cosign")
            self.assertEqual(command[1], "verify-blob")
            self.assertIn("--bundle", command)
            self.assertEqual(command[command.index("--bundle") + 1], str(Path(raw) / bundle_name(asset)))
            self.assertEqual(command[command.index("--certificate-identity") + 1], identity)
            self.assertEqual(
                command[command.index("--certificate-oidc-issuer") + 1],
                "https://token.actions.githubusercontent.com",
            )
            self.assertEqual(command[-1], str(Path(raw) / asset))

    def test_verify_signed_cli_assets_surfaces_a_cosign_failure(self):
        runner = _FakeRunner(fail_on="PapyrusLinterCLI-macos")
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            _write_signed_assets(directory)

            with self.assertRaisesRegex(VerificationError, "PapyrusLinterCLI-macos"):
                verify_signed_cli_assets(directory, identity="https://example.test", runner=runner)

    def test_verify_signed_cli_assets_surfaces_a_missing_cosign_binary(self):
        runner = _FakeRunner(missing_binary=True)
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            _write_signed_assets(directory)

            with self.assertRaisesRegex(VerificationError, "failed to run cosign"):
                verify_signed_cli_assets(directory, identity="https://example.test", runner=runner)

    def test_main_verifies_then_bakes_hashes(self):
        runner = _FakeRunner()
        identity = "https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v9.9.9"
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            bodies = _write_signed_assets(directory)
            ts_path = directory / "cliHashes.ts"
            py_path = directory / "cli_hashes.py"
            argv = [
                "verify_cli_signatures.py",
                str(directory),
                "--certificate-identity",
                identity,
                "--ts-path",
                str(ts_path),
                "--py-path",
                str(py_path),
            ]
            with (
                patch("sys.argv", argv),
                patch("verify_cli_signatures.subprocess.run", runner),
            ):
                self.assertEqual(main(), 0)

            ts = ts_path.read_text(encoding="utf-8")
            py = py_path.read_text(encoding="utf-8")
            for body in bodies.values():
                digest = hashlib.sha256(body).hexdigest()
                self.assertIn(digest, ts)
                self.assertIn(digest, py)
        self.assertEqual(len(runner.calls), len(ASSETS))

    def test_main_requires_a_certificate_identity(self):
        with tempfile.TemporaryDirectory() as raw, patch("sys.argv", ["verify_cli_signatures.py", raw]):
            env = {key: value for key, value in os.environ.items() if key != "CERTIFICATE_IDENTITY"}
            with patch.dict(os.environ, env, clear=True):
                self.assertEqual(main(), 2)

    def test_main_returns_one_when_verification_fails(self):
        runner = _FakeRunner(fail_on="PapyrusLinterCLI-linux")
        with tempfile.TemporaryDirectory() as raw:
            directory = Path(raw)
            _write_signed_assets(directory)
            argv = [
                "verify_cli_signatures.py",
                str(directory),
                "--certificate-identity",
                "https://example.test",
            ]
            with (
                patch("sys.argv", argv),
                patch("verify_cli_signatures.subprocess.run", runner),
            ):
                self.assertEqual(main(), 1)


if __name__ == "__main__":
    unittest.main()

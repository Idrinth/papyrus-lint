"""Tests for release-specific CLI download selection and caching."""

from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import importlib.util
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
sublime = types.ModuleType('sublime')
sublime.load_resource = unittest.mock.Mock(side_effect=FileNotFoundError)
with unittest.mock.patch.dict(sys.modules, {'sublime': sublime}):
    SPEC = importlib.util.spec_from_file_location('cli_download_test', ROOT / 'cli_download.py')
    cli_download = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(cli_download)


class CliDownloadTests(unittest.TestCase):
    def test_selects_release_asset_for_each_supported_platform(self):
        self.assertEqual(cli_download._asset_name('Windows'), 'PapyrusLinterCLI-windows.exe')
        self.assertEqual(cli_download._asset_name('Darwin'), 'PapyrusLinterCLI-macos')
        self.assertEqual(cli_download._asset_name('Linux'), 'PapyrusLinterCLI-linux')

    def test_rejects_an_unsupported_platform(self):
        with self.assertRaisesRegex(OSError, 'FreeBSD'):
            cli_download._asset_name('FreeBSD')

    def test_package_control_version_takes_precedence(self):
        with patch.object(
            cli_download.sublime,
            'load_resource',
            return_value='{"version": "v2.3.4"}',
        ):
            self.assertEqual(cli_download.release_version(), '2.3.4')

    def test_reuses_a_cached_executable(self):
        with tempfile.TemporaryDirectory() as cache:
            executable = Path(cache) / 'PapyrusLint' / 'v1.2.3' / 'PapyrusLinterCLI-linux'
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b'cli')
            executable.chmod(0o700)
            with patch.object(cli_download, 'urlopen') as download:
                result = cli_download.ensure_release_cli(cache, '1.2.3', 'Linux')

            self.assertEqual(result, str(executable))
            download.assert_not_called()


if __name__ == '__main__':
    unittest.main()

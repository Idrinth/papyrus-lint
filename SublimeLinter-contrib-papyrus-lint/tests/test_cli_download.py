"""Tests for release-specific CLI download selection and caching."""

from io import BytesIO
import os
from pathlib import Path, PosixPath
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

    def test_detects_the_current_platform_when_none_is_supplied(self):
        with patch.object(cli_download.platform, 'system', return_value='Darwin') as system:
            self.assertEqual(cli_download._asset_name(), 'PapyrusLinterCLI-macos')

        system.assert_called_once_with()

    def test_package_control_version_takes_precedence(self):
        with patch.object(
            cli_download.sublime,
            'load_resource',
            return_value='{"version": "v2.3.4"}',
        ):
            self.assertEqual(cli_download.release_version(), '2.3.4')

    def test_version_falls_back_to_bundled_file_for_unusable_metadata(self):
        unusable_metadata = (
            FileNotFoundError(),
            OSError('resource unavailable'),
            '{not json',
            '{}',
        )
        for metadata in unusable_metadata:
            with self.subTest(metadata=metadata):
                if isinstance(metadata, Exception):
                    resource = patch.object(
                        cli_download.sublime,
                        'load_resource',
                        side_effect=metadata,
                    )
                else:
                    resource = patch.object(
                        cli_download.sublime,
                        'load_resource',
                        return_value=metadata,
                    )
                with resource:
                    self.assertEqual(cli_download.release_version(), '0.1.0')

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

    def test_reuses_a_windows_cache_without_unix_execute_permissions(self):
        with tempfile.TemporaryDirectory() as cache:
            executable = (
                Path(cache)
                / 'PapyrusLint'
                / 'v1.2.3'
                / 'PapyrusLinterCLI-windows.exe'
            )
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b'cli')
            executable.chmod(0o600)

            with (
                patch.object(cli_download.os, 'name', 'nt'),
                patch.object(cli_download, 'Path', PosixPath),
                patch.object(cli_download, 'urlopen') as download,
            ):
                result = cli_download.ensure_release_cli(
                    cache, '1.2.3', 'Windows'
                )

            self.assertEqual(result, str(executable))
            download.assert_not_called()

    def test_uses_release_version_and_detected_platform_by_default(self):
        with (
            tempfile.TemporaryDirectory() as cache,
            patch.object(cli_download, 'release_version', return_value='4.5.6') as version,
            patch.object(cli_download.platform, 'system', return_value='Linux') as system,
            patch.object(cli_download, 'urlopen', return_value=BytesIO(b'cli')) as download,
        ):
            result = Path(cli_download.ensure_release_cli(cache))

            self.assertEqual(
                result,
                Path(cache) / 'PapyrusLint' / 'v4.5.6' / 'PapyrusLinterCLI-linux',
            )

        version.assert_called_once_with()
        system.assert_called_once_with()
        download.assert_called_once_with(
            '{}/v4.5.6/PapyrusLinterCLI-linux'.format(cli_download.RELEASE_BASE),
            timeout=30,
        )

    def test_downloads_release_to_versioned_cache_and_makes_it_executable(self):
        with tempfile.TemporaryDirectory() as cache:
            response = BytesIO(b'first chunk' + b'second chunk')
            with patch.object(cli_download, 'urlopen', return_value=response) as download:
                result = Path(
                    cli_download.ensure_release_cli(cache, '2.3.4', 'Linux')
                )

            self.assertEqual(
                result,
                Path(cache)
                / 'PapyrusLint'
                / 'v2.3.4'
                / 'PapyrusLinterCLI-linux',
            )
            self.assertEqual(result.read_bytes(), b'first chunksecond chunk')
            self.assertTrue(os.access(result, os.X_OK))
            download.assert_called_once_with(
                '{}/v2.3.4/PapyrusLinterCLI-linux'.format(
                    cli_download.RELEASE_BASE
                ),
                timeout=30,
            )
            self.assertEqual(list(result.parent.iterdir()), [result])

    def test_failed_download_removes_temporary_file(self):
        with tempfile.TemporaryDirectory() as cache:
            with patch.object(
                cli_download,
                'urlopen',
                side_effect=OSError('offline'),
            ):
                with self.assertRaisesRegex(OSError, 'offline'):
                    cli_download.ensure_release_cli(cache, '2.3.4', 'Linux')

            directory = Path(cache) / 'PapyrusLint' / 'v2.3.4'
            self.assertEqual(list(directory.iterdir()), [])

    def test_failed_cache_install_removes_temporary_file(self):
        with tempfile.TemporaryDirectory() as cache:
            with (
                patch.object(cli_download, 'urlopen', return_value=BytesIO(b'cli')),
                patch.object(cli_download.os, 'replace', side_effect=OSError('disk full')),
            ):
                with self.assertRaisesRegex(OSError, 'disk full'):
                    cli_download.ensure_release_cli(cache, '2.3.4', 'Linux')

            directory = Path(cache) / 'PapyrusLint' / 'v2.3.4'
            self.assertEqual(list(directory.iterdir()), [])

    def test_non_executable_cached_file_is_replaced_on_unix(self):
        with tempfile.TemporaryDirectory() as cache:
            executable = (
                Path(cache)
                / 'PapyrusLint'
                / 'v1.2.3'
                / 'PapyrusLinterCLI-linux'
            )
            executable.parent.mkdir(parents=True)
            executable.write_bytes(b'stale')
            executable.chmod(0o600)

            with patch.object(
                cli_download, 'urlopen', return_value=BytesIO(b'fresh')
            ) as download:
                result = cli_download.ensure_release_cli(cache, '1.2.3', 'Linux')

            self.assertEqual(result, str(executable))
            self.assertEqual(executable.read_bytes(), b'fresh')
            download.assert_called_once()


if __name__ == '__main__':
    unittest.main()

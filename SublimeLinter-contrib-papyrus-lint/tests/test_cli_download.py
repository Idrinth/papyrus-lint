"""Tests for release-specific CLI download selection and caching."""

import importlib.util
import os
import sys
import tempfile
import types
import unittest
from io import BytesIO
from pathlib import Path, PosixPath
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sublime = types.ModuleType('sublime')
sublime.load_resource = unittest.mock.Mock(side_effect=FileNotFoundError)
with unittest.mock.patch.dict(sys.modules, {'sublime': sublime}):
    SPEC = importlib.util.spec_from_file_location('cli_download_test', ROOT / 'cli_download.py')
    cli_download = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(cli_download)


class CliDownloadTests(unittest.TestCase):
    def setUp(self):
        cli_download._verified_executables.clear()

    def test_verifies_and_caches_a_matching_manually_configured_cli(self):
        completed = unittest.mock.Mock(
            stdout='PapyrusLinterCLI 0.1.0\n', stderr=''
        )
        with patch.object(cli_download.subprocess, 'run', return_value=completed) as run:
            cli_download.verify_configured_cli('/tools/PapyrusLinterCLI')
            cli_download.verify_configured_cli('/tools/PapyrusLinterCLI')

        run.assert_called_once_with(
            ['/tools/PapyrusLinterCLI', '--version'],
            check=True,
            capture_output=True,
            text=True,
        )

    def test_rejects_a_mismatched_manually_configured_cli(self):
        completed = unittest.mock.Mock(
            stdout='PapyrusLinterCLI 0.0.9\n', stderr=''
        )
        with (
            patch.object(cli_download.subprocess, 'run', return_value=completed),
            self.assertRaisesRegex(OSError, 'expected "PapyrusLinterCLI 0.1.0"'),
        ):
            cli_download.verify_configured_cli('/tools/PapyrusLinterCLI')

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

    def test_on_disk_package_metadata_wins_over_stale_load_resource(self):
        with tempfile.TemporaryDirectory() as package:
            Path(package, 'package-metadata.json').write_text(
                '{"version": "v9.8.7"}', encoding='utf-8'
            )
            with (
                patch.object(cli_download, '_package_dir', return_value=Path(package)),
                patch.object(
                    cli_download.sublime,
                    'load_resource',
                    return_value='{"version": "v1.0.0"}',
                ),
            ):
                self.assertEqual(cli_download.release_version(), '9.8.7')

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
            f'{cli_download.RELEASE_BASE}/v4.5.6/PapyrusLinterCLI-linux',
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
                f'{cli_download.RELEASE_BASE}/v2.3.4/PapyrusLinterCLI-linux',
                timeout=30,
            )
            self.assertEqual(list(result.parent.iterdir()), [result])

    def test_downloads_a_new_cli_when_the_plugin_version_changes(self):
        with tempfile.TemporaryDirectory() as cache:
            previous = Path(cache) / 'PapyrusLint' / 'v1.2.3' / 'PapyrusLinterCLI-linux'
            previous.parent.mkdir(parents=True)
            previous.write_bytes(b'old')
            previous.chmod(0o700)
            leftover = Path(cache) / 'PapyrusLint' / 'notes.txt'
            leftover.write_text('keep', encoding='utf-8')

            with patch.object(cli_download, 'urlopen', return_value=BytesIO(b'fresh')) as download:
                result = Path(cli_download.ensure_release_cli(cache, '1.2.4', 'Linux'))

            self.assertEqual(
                result,
                Path(cache) / 'PapyrusLint' / 'v1.2.4' / 'PapyrusLinterCLI-linux',
            )
            self.assertEqual(result.read_bytes(), b'fresh')
            download.assert_called_once_with(
                f'{cli_download.RELEASE_BASE}/v1.2.4/PapyrusLinterCLI-linux',
                timeout=30,
            )
            self.assertFalse(previous.exists())
            self.assertEqual(leftover.read_text(encoding='utf-8'), 'keep')

    def test_cache_hit_still_removes_previous_version_binaries(self):
        with tempfile.TemporaryDirectory() as cache:
            current = Path(cache) / 'PapyrusLint' / 'v2.0.0' / 'PapyrusLinterCLI-linux'
            previous = Path(cache) / 'PapyrusLint' / 'v1.9.0' / 'PapyrusLinterCLI-linux'
            current.parent.mkdir(parents=True)
            previous.parent.mkdir(parents=True)
            current.write_bytes(b'current')
            current.chmod(0o700)
            previous.write_bytes(b'previous')
            previous.chmod(0o700)

            with patch.object(cli_download, 'urlopen') as download:
                result = cli_download.ensure_release_cli(cache, '2.0.0', 'Linux')

            self.assertEqual(result, str(current))
            download.assert_not_called()
            self.assertFalse(previous.exists())
            self.assertEqual(current.read_bytes(), b'current')

    def test_plugin_loaded_prefetches_in_a_background_thread(self):
        with patch.object(cli_download.threading, 'Thread') as thread:
            cli_download.plugin_loaded()

        thread.assert_called_once_with(target=cli_download.prefetch_release_cli, daemon=True)
        thread.return_value.start.assert_called_once_with()

    def test_prefetch_swallows_download_failures(self):
        with patch.object(cli_download, 'ensure_release_cli', side_effect=OSError('offline')):
            self.assertIsNone(cli_download.prefetch_release_cli('/cache'))

    def test_prefetch_uses_sublime_cache_path_by_default(self):
        with (
            patch.object(cli_download.sublime, 'cache_path', create=True, return_value='/cache') as cache_path,
            patch.object(cli_download, 'ensure_release_cli') as ensure,
        ):
            cli_download.prefetch_release_cli()

        cache_path.assert_called_once_with()
        ensure.assert_called_once_with('/cache')

    def test_pruning_a_missing_cache_directory_is_a_no_op(self):
        cli_download._prune_other_versions('/definitely-not-a-papyrus-lint-cache', '1.0.0')

    def test_failed_download_removes_temporary_file(self):
        with tempfile.TemporaryDirectory() as cache:
            with patch.object(
                cli_download,
                'urlopen',
                side_effect=OSError('offline'),
            ), self.assertRaisesRegex(OSError, 'offline'):
                cli_download.ensure_release_cli(cache, '2.3.4', 'Linux')

            directory = Path(cache) / 'PapyrusLint' / 'v2.3.4'
            self.assertEqual(list(directory.iterdir()), [])

    def test_failed_cache_install_removes_temporary_file(self):
        with tempfile.TemporaryDirectory() as cache:
            with (
                patch.object(cli_download, 'urlopen', return_value=BytesIO(b'cli')),
                patch.object(cli_download.os, 'replace', side_effect=OSError('disk full')),
                self.assertRaisesRegex(OSError, 'disk full'),
            ):
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

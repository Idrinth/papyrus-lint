"""Tests for the Sublime Text fix command using lightweight API stubs."""

import importlib.util
import json
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import Mock, patch


PLUGIN_ROOT = Path(__file__).resolve().parents[1]


class FakeTextCommand:
    def __init__(self, view):
        self.view = view


def load_commands_module(settings=None):
    if settings is None:
        settings = Mock()
        settings.get.side_effect = lambda _key, default=None: default
    sublime = types.ModuleType('sublime')
    sublime.error_message = Mock()
    sublime.load_settings = Mock(return_value=settings)
    sublime.cache_path = Mock(return_value='/cache')

    sublime_plugin = types.ModuleType('sublime_plugin')
    sublime_plugin.TextCommand = FakeTextCommand
    plugin_package = types.ModuleType('papyrus_lint_plugin')
    plugin_package.__path__ = [str(PLUGIN_ROOT)]

    with patch.dict(
        sys.modules,
        {
            'sublime': sublime,
            'sublime_plugin': sublime_plugin,
            'papyrus_lint_plugin': plugin_package,
        },
    ):
        spec = importlib.util.spec_from_file_location(
            'papyrus_lint_plugin.commands', PLUGIN_ROOT / 'commands.py'
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.ensure_release_cli = Mock(return_value='/cache/PapyrusLinterCLI')
    return module, sublime


class PapyrusLintFixCommandTests(unittest.TestCase):
    def setUp(self):
        self.module, self.sublime = load_commands_module()
        self.view = Mock()
        self.view.file_name.return_value = '/scripts/Example.psc'
        self.view.is_dirty.return_value = False
        self.command = self.module.PapyrusLintFixCommand(self.view)

    def test_visibility_and_enabled_state_require_a_saved_clean_psc_file(self):
        self.assertTrue(self.command.is_visible())
        self.assertTrue(self.command.is_enabled())

        self.view.file_name.return_value = '/scripts/Example.txt'
        self.assertFalse(self.command.is_visible())

        self.view.file_name.return_value = None
        self.assertFalse(self.command.is_visible())
        self.assertFalse(self.command.is_enabled())

        self.view.file_name.return_value = '/scripts/Example.PSC'
        self.view.is_dirty.return_value = True
        self.assertTrue(self.command.is_visible())
        self.assertFalse(self.command.is_enabled())

    def test_successful_fix_reverts_and_relints(self):
        result = Mock(returncode=1)
        with patch.object(self.module.subprocess, 'run', return_value=result) as run:
            self.command.run(None)

        run.assert_called_once_with(
            ('/cache/PapyrusLinterCLI', 'fix', '/scripts/Example.psc'),
            capture_output=True,
            startupinfo=None,
        )
        self.assertEqual(
            self.view.run_command.call_args_list,
            [unittest.mock.call('revert'), unittest.mock.call('sublime_linter_lint')],
        )
        self.sublime.error_message.assert_not_called()

    def test_windows_fix_hides_the_cli_window(self):
        startupinfo = Mock(dwFlags=4)
        self.module.subprocess.STARTUPINFO = Mock(return_value=startupinfo)
        self.module.subprocess.STARTF_USESHOWWINDOW = 2

        with (
            patch.object(self.module.os, 'name', 'nt'),
            patch.object(
                self.module.subprocess,
                'run',
                return_value=Mock(returncode=0),
            ) as run,
        ):
            self.command.run(None)

        self.module.subprocess.STARTUPINFO.assert_called_once_with()
        self.assertEqual(startupinfo.dwFlags, 6)
        self.assertIs(run.call_args.kwargs['startupinfo'], startupinfo)

    def test_usage_or_io_failure_shows_decoded_stderr(self):
        result = Mock(returncode=2, stderr=b'bad arguments\xff')
        with patch.object(self.module.subprocess, 'run', return_value=result):
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: fix failed:\nbad arguments\ufffd'
        )
        self.view.run_command.assert_not_called()

    def test_empty_failure_message_has_a_fallback(self):
        result = Mock(returncode=2, stderr=b'')
        with patch.object(self.module.subprocess, 'run', return_value=result):
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: fix failed:\nunknown error'
        )

    def test_os_error_is_reported_without_reloading(self):
        with patch.object(
            self.module.subprocess, 'run', side_effect=OSError('not found')
        ):
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: not found'
        )
        self.view.run_command.assert_not_called()

    def test_download_error_is_reported_without_running_or_reloading(self):
        self.module.ensure_release_cli.side_effect = OSError('offline')
        with patch.object(self.module.subprocess, 'run') as run:
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: offline'
        )
        run.assert_not_called()
        self.view.run_command.assert_not_called()

    def test_missing_file_is_a_no_op(self):
        self.view.file_name.return_value = None
        with patch.object(self.module.subprocess, 'run') as run:
            self.command.run(None)

        run.assert_not_called()

    def test_configured_executable_is_used(self):
        settings = Mock()
        settings.get.return_value = {
            'papyrus-lint': {'executable': '/tools/custom-linter'}
        }
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixCommand(self.view)

        self.assertEqual(command._executable(), '/tools/custom-linter')

    def test_default_executable_is_used_when_not_configured(self):
        settings = Mock()
        settings.get.return_value = {'papyrus-lint': {}}
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixCommand(self.view)

        self.assertEqual(command._executable(), '/cache/PapyrusLinterCLI')

    def test_config_path_defaults_to_empty(self):
        settings = Mock()
        settings.get.return_value = {'papyrus-lint': {}}
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixCommand(self.view)

        self.assertEqual(command._config_path(), '')

    def test_config_path_is_trimmed(self):
        settings = Mock()
        settings.get.return_value = {'papyrus-lint': {'config_path': '  /project/custom.yaml  '}}
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixCommand(self.view)

        self.assertEqual(command._config_path(), '/project/custom.yaml')

    def test_fix_inserts_config_flag_when_configured(self):
        settings = Mock()
        settings.get.return_value = {
            'papyrus-lint': {'config_path': '/project/custom.yaml'}
        }
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixCommand(self.view)
        result = Mock(returncode=0)

        with patch.object(module.subprocess, 'run', return_value=result) as run:
            command.run(None)

        run.assert_called_once_with(
            (
                '/cache/PapyrusLinterCLI',
                'fix',
                '--config',
                '/project/custom.yaml',
                '/scripts/Example.psc',
            ),
            capture_output=True,
            startupinfo=None,
        )


def _json_result(report, returncode=1):
    return Mock(returncode=returncode, stdout=json.dumps(report).encode('utf-8'))


class PapyrusLintFixIssueCommandTests(unittest.TestCase):
    def setUp(self):
        self.module, self.sublime = load_commands_module()
        self.view = Mock()
        self.view.file_name.return_value = '/scripts/Example.psc'
        self.view.is_dirty.return_value = False
        selection = Mock()
        selection.begin.return_value = 42
        self.view.sel.return_value = [selection]
        # Caret on line 4 (0-based row 3), column 7 (0-based col 6).
        self.view.rowcol.return_value = (3, 6)
        self.command = self.module.PapyrusLintFixIssueCommand(self.view)
        self.report = {
            'files': [
                {
                    'path': '/scripts/Example.psc',
                    'diagnostics': [
                        {'line': 4, 'column': 7, 'rule': 'trailing-whitespace'},
                    ],
                }
            ]
        }

    def test_visibility_and_enabled_state_require_a_saved_clean_psc_file(self):
        self.assertTrue(self.command.is_visible())
        self.assertTrue(self.command.is_enabled())

        self.view.file_name.return_value = '/scripts/Example.txt'
        self.assertFalse(self.command.is_visible())

        self.view.file_name.return_value = None
        self.assertFalse(self.command.is_visible())
        self.assertFalse(self.command.is_enabled())

        self.view.file_name.return_value = '/scripts/Example.PSC'
        self.view.is_dirty.return_value = True
        self.assertTrue(self.command.is_visible())
        self.assertFalse(self.command.is_enabled())

    def test_fixes_only_the_rule_and_line_of_the_nearest_diagnostic(self):
        report_result = _json_result(self.report)
        fix_result = Mock(returncode=0)
        with patch.object(
            self.module.subprocess, 'run', side_effect=[report_result, fix_result]
        ) as run:
            self.command.run(None)

        self.assertEqual(
            run.call_args_list[0],
            unittest.mock.call(
                ('/cache/PapyrusLinterCLI', '--json', '/scripts/Example.psc'),
                capture_output=True,
                startupinfo=None,
            ),
        )
        self.assertEqual(
            run.call_args_list[1],
            unittest.mock.call(
                (
                    '/cache/PapyrusLinterCLI',
                    'fix',
                    '--type',
                    'trailing-whitespace',
                    '--line',
                    '4',
                    '/scripts/Example.psc',
                ),
                capture_output=True,
                startupinfo=None,
            ),
        )
        self.assertEqual(
            self.view.run_command.call_args_list,
            [unittest.mock.call('revert'), unittest.mock.call('sublime_linter_lint')],
        )
        self.sublime.error_message.assert_not_called()

    def test_picks_the_diagnostic_closest_to_the_caret_column(self):
        report = {
            'files': [
                {
                    'path': '/scripts/Example.psc',
                    'diagnostics': [
                        {'line': 4, 'column': 1, 'rule': 'far-rule'},
                        {'line': 4, 'column': 8, 'rule': 'near-rule'},
                        {'line': 9, 'column': 7, 'rule': 'other-line-rule'},
                    ],
                }
            ]
        }
        report_result = _json_result(report)
        fix_result = Mock(returncode=0)
        with patch.object(
            self.module.subprocess, 'run', side_effect=[report_result, fix_result]
        ) as run:
            self.command.run(None)

        self.assertIn('near-rule', run.call_args_list[1].args[0])

    def test_no_issue_on_the_caret_line_shows_a_message_without_fixing(self):
        report = {'files': [{'path': '/scripts/Example.psc', 'diagnostics': []}]}
        report_result = _json_result(report, returncode=0)
        with patch.object(
            self.module.subprocess, 'run', return_value=report_result
        ) as run:
            self.command.run(None)

        run.assert_called_once()
        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: no issue reported on the current line.'
        )
        self.view.run_command.assert_not_called()

    def test_no_selection_is_a_no_op(self):
        self.view.sel.return_value = []
        with patch.object(self.module.subprocess, 'run') as run:
            self.command.run(None)

        run.assert_not_called()

    def test_missing_file_is_a_no_op(self):
        self.view.file_name.return_value = None
        with patch.object(self.module.subprocess, 'run') as run:
            self.command.run(None)

        run.assert_not_called()

    def test_invalid_json_report_is_treated_as_no_issue(self):
        report_result = Mock(returncode=0, stdout=b'not JSON')
        with patch.object(
            self.module.subprocess, 'run', return_value=report_result
        ):
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: no issue reported on the current line.'
        )
        self.view.run_command.assert_not_called()

    def test_report_read_failure_shows_decoded_stderr_without_fixing(self):
        report_result = Mock(returncode=2, stderr=b'bad arguments')
        with patch.object(
            self.module.subprocess, 'run', return_value=report_result
        ) as run:
            self.command.run(None)

        run.assert_called_once()
        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: read diagnostics failed:\nbad arguments'
        )
        self.view.run_command.assert_not_called()

    def test_fix_failure_shows_decoded_stderr(self):
        report_result = _json_result(self.report)
        fix_result = Mock(returncode=2, stderr=b'bad arguments')
        with patch.object(
            self.module.subprocess, 'run', side_effect=[report_result, fix_result]
        ):
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: fix failed:\nbad arguments'
        )
        self.view.run_command.assert_not_called()

    def test_os_error_reading_diagnostics_is_reported_without_fixing(self):
        with patch.object(
            self.module.subprocess, 'run', side_effect=OSError('not found')
        ) as run:
            self.command.run(None)

        run.assert_called_once()
        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: not found'
        )
        self.view.run_command.assert_not_called()

    def test_download_error_is_reported_without_running(self):
        self.module.ensure_release_cli.side_effect = OSError('offline')
        with patch.object(self.module.subprocess, 'run') as run:
            self.command.run(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: offline'
        )
        run.assert_not_called()
        self.view.run_command.assert_not_called()

    def test_fix_inserts_config_flag_when_configured(self):
        settings = Mock()
        settings.get.return_value = {
            'papyrus-lint': {'config_path': '/project/custom.yaml'}
        }
        module, _ = load_commands_module(settings)
        command = module.PapyrusLintFixIssueCommand(self.view)
        report_result = _json_result(self.report)
        fix_result = Mock(returncode=0)

        with patch.object(
            module.subprocess, 'run', side_effect=[report_result, fix_result]
        ) as run:
            command.run(None)

        self.assertEqual(
            run.call_args_list[0],
            unittest.mock.call(
                (
                    '/cache/PapyrusLinterCLI',
                    '--json',
                    '--config',
                    '/project/custom.yaml',
                    '/scripts/Example.psc',
                ),
                capture_output=True,
                startupinfo=None,
            ),
        )
        self.assertEqual(
            run.call_args_list[1],
            unittest.mock.call(
                (
                    '/cache/PapyrusLinterCLI',
                    'fix',
                    '--type',
                    'trailing-whitespace',
                    '--line',
                    '4',
                    '--config',
                    '/project/custom.yaml',
                    '/scripts/Example.psc',
                ),
                capture_output=True,
                startupinfo=None,
            ),
        )


if __name__ == '__main__':
    unittest.main()

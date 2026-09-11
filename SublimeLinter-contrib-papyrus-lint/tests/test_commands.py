"""Tests for the Sublime Text fix command using lightweight API stubs."""

import importlib.util
import json
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

PLUGIN_ROOT = Path(__file__).resolve().parents[1]


class FakeTextCommand:
    def __init__(self, view):
        self.view = view


class FakeWindowCommand:
    def __init__(self, window):
        self.window = window


def load_commands_module(settings=None):
    if settings is None:
        settings = Mock()
        settings.get.side_effect = lambda _key, default=None: default
    sublime = types.ModuleType('sublime')
    sublime.error_message = Mock()
    sublime.status_message = Mock()
    sublime.load_settings = Mock(return_value=settings)
    sublime.cache_path = Mock(return_value='/cache')

    sublime_plugin = types.ModuleType('sublime_plugin')
    sublime_plugin.TextCommand = FakeTextCommand
    sublime_plugin.WindowCommand = FakeWindowCommand
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


class PapyrusLintInitCommandTests(unittest.TestCase):
    def setUp(self):
        self.module, self.sublime = load_commands_module()
        self.window = Mock()
        self.command = self.module.PapyrusLintInitCommand(self.window)

    def test_starts_directly_when_a_single_folder_is_open(self):
        self.window.folders.return_value = ['/project']

        self.command.run()

        self.window.show_quick_panel.assert_called_once_with(
            self.module.PapyrusLintInitCommand.PRESET_LABELS,
            self.command._on_preset_chosen,
        )
        self.assertEqual(self.command._directory, '/project')

    def test_prompts_for_a_folder_when_several_are_open(self):
        self.window.folders.return_value = ['/one', '/two']

        self.command.run()

        (folders, callback), _kwargs = self.window.show_quick_panel.call_args
        self.assertEqual(folders, ['/one', '/two'])

        self.window.show_quick_panel.reset_mock()
        callback(1)

        self.assertEqual(self.command._directory, '/two')
        self.window.show_quick_panel.assert_called_once_with(
            self.module.PapyrusLintInitCommand.PRESET_LABELS,
            self.command._on_preset_chosen,
        )

    def test_cancelling_the_folder_prompt_is_a_no_op(self):
        self.window.folders.return_value = ['/one', '/two']

        self.command.run()
        (_folders, callback), _kwargs = self.window.show_quick_panel.call_args
        self.window.show_quick_panel.reset_mock()
        callback(-1)

        self.window.show_quick_panel.assert_not_called()

    def test_falls_back_to_the_active_view_directory_when_no_folder_is_open(self):
        self.window.folders.return_value = []
        view = Mock()
        view.file_name.return_value = '/scripts/Example.psc'
        self.window.active_view.return_value = view

        self.command.run()

        self.assertEqual(self.command._directory, '/scripts')
        self.window.show_quick_panel.assert_called_once()

    def test_shows_an_error_when_no_folder_or_saved_file_is_available(self):
        self.window.folders.return_value = []
        self.window.active_view.return_value = None

        self.command.run()

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: open a folder or a saved file first.'
        )
        self.window.show_quick_panel.assert_not_called()

    def test_cancelling_the_preset_panel_is_a_no_op(self):
        with patch.object(self.command, '_run_init') as run_init:
            self.command._on_preset_chosen(-1)

        run_init.assert_not_called()

    def test_choosing_a_built_in_preset_runs_init_with_its_value(self):
        with patch.object(self.command, '_run_init') as run_init:
            self.command._on_preset_chosen(0)
            self.command._on_preset_chosen(1)
            self.command._on_preset_chosen(2)

        self.assertEqual(
            run_init.call_args_list,
            [unittest.mock.call(None), unittest.mock.call('standard'), unittest.mock.call('careful')],
        )

    def test_choosing_custom_preset_shows_an_input_panel(self):
        self.command._on_preset_chosen(3)

        self.window.show_input_panel.assert_called_once_with(
            'Custom preset name:', '', self.command._on_custom_preset_entered, None, None
        )

    def test_blank_custom_preset_is_a_no_op(self):
        with patch.object(self.command, '_run_init') as run_init:
            self.command._on_custom_preset_entered('   ')

        run_init.assert_not_called()

    def test_custom_preset_is_trimmed_before_running_init(self):
        with patch.object(self.command, '_run_init') as run_init:
            self.command._on_custom_preset_entered('  team-style  ')

        run_init.assert_called_once_with('team-style')

    def test_successful_init_without_a_preset_shows_the_created_path(self):
        self.command._directory = '/project'
        result = Mock(returncode=0, stdout=b'Created /project/papyrus-lint.yaml\n')

        with patch.object(self.module.subprocess, 'run', return_value=result) as run:
            self.command._run_init(None)

        run.assert_called_once_with(
            ('/cache/PapyrusLinterCLI', 'init'),
            capture_output=True,
            cwd='/project',
            startupinfo=None,
        )
        self.sublime.status_message.assert_called_once_with('Created /project/papyrus-lint.yaml')
        self.sublime.error_message.assert_not_called()

    def test_successful_init_with_a_preset_passes_it_through(self):
        self.command._directory = '/project'
        result = Mock(returncode=0, stdout=b'Created /project/papyrus-lint.yaml\n')

        with patch.object(self.module.subprocess, 'run', return_value=result) as run:
            self.command._run_init('careful')

        run.assert_called_once_with(
            ('/cache/PapyrusLinterCLI', 'init', '--preset', 'careful'),
            capture_output=True,
            cwd='/project',
            startupinfo=None,
        )

    def test_init_failure_shows_decoded_stderr(self):
        self.command._directory = '/project'
        result = Mock(returncode=2, stderr=b'error: config already exists')

        with patch.object(self.module.subprocess, 'run', return_value=result):
            self.command._run_init(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: init failed:\nerror: config already exists'
        )
        self.sublime.status_message.assert_not_called()

    def test_init_failure_with_no_stderr_has_a_fallback_message(self):
        self.command._directory = '/project'
        result = Mock(returncode=2, stderr=b'')

        with patch.object(self.module.subprocess, 'run', return_value=result):
            self.command._run_init(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: init failed:\nunknown error'
        )

    def test_os_error_running_the_cli_is_reported(self):
        self.command._directory = '/project'

        with patch.object(
            self.module.subprocess, 'run', side_effect=OSError('not found')
        ):
            self.command._run_init(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: not found'
        )
        self.sublime.status_message.assert_not_called()

    def test_download_error_is_reported_without_running(self):
        self.command._directory = '/project'
        self.module.ensure_release_cli.side_effect = OSError('offline')

        with patch.object(self.module.subprocess, 'run') as run:
            self.command._run_init(None)

        self.sublime.error_message.assert_called_once_with(
            'PapyrusLint: failed to download or run the CLI: offline'
        )
        run.assert_not_called()

    def test_configured_executable_is_used(self):
        settings = Mock()
        settings.get.return_value = {'papyrus-lint': {'executable': '/tools/custom-linter'}}
        module, _sublime = load_commands_module(settings)
        window = Mock()
        command = module.PapyrusLintInitCommand(window)
        command._directory = '/project'
        result = Mock(returncode=0, stdout=b'Created /project/papyrus-lint.yaml\n')

        with patch.object(module.subprocess, 'run', return_value=result) as run:
            command._run_init(None)

        run.assert_called_once_with(
            ('/tools/custom-linter', 'init'),
            capture_output=True,
            cwd='/project',
            startupinfo=None,
        )


if __name__ == '__main__':
    unittest.main()

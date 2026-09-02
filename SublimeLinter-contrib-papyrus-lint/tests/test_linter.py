"""Tests for the SublimeLinter adapter without requiring Sublime Text."""

import importlib.util
import json
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import Mock, patch


PLUGIN_ROOT = Path(__file__).resolve().parents[1]


class FakeLintMatch:
    """Small stand-in that exposes the fields supplied to LintMatch."""

    def __init__(self, **kwargs):
        self.__dict__.update(kwargs)


class FakeLinter:
    def __init__(self, settings=None):
        self.logger = Mock()
        self.notify_failure = Mock()
        self.name = 'papyrus-lint'
        self.settings = settings if settings is not None else {}


class FakePermanentError(Exception):
    pass


def load_linter_module():
    lint_module = types.ModuleType('SublimeLinter.lint')
    lint_module.Linter = FakeLinter
    lint_module.LintMatch = FakeLintMatch
    lint_module.PermanentError = FakePermanentError

    package = types.ModuleType('SublimeLinter')
    package.lint = lint_module
    sublime = types.ModuleType('sublime')
    sublime.cache_path = lambda: '/tmp/sublime-cache'

    plugin_package = types.ModuleType('papyrus_lint_plugin')
    plugin_package.__path__ = [str(PLUGIN_ROOT)]

    with unittest.mock.patch.dict(
        sys.modules,
        {
            'SublimeLinter': package,
            'SublimeLinter.lint': lint_module,
            'sublime': sublime,
            'papyrus_lint_plugin': plugin_package,
        },
    ):
        spec = importlib.util.spec_from_file_location(
            'papyrus_lint_plugin.linter', PLUGIN_ROOT / 'linter.py'
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    return module


class PapyrusLintTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = load_linter_module()

    def setUp(self):
        self.linter = self.module.PapyrusLint()

    def test_command_and_selector_target_saved_papyrus_files(self):
        self.assertEqual(self.linter.executable, 'PapyrusLinterCLI')
        self.assertEqual(self.linter.defaults['selector'], 'source.papyrus')
        self.assertEqual(self.linter.defaults['config_path'], '')

    def test_cmd_inserts_config_flag_when_config_path_is_set(self):
        self.linter.settings = {
            'executable': '/tools/PapyrusLinterCLI',
            'config_path': '/project/custom-lint.yaml',
        }

        self.assertEqual(
            self.linter.cmd(),
            ['/tools/PapyrusLinterCLI', '--json', '--config', '/project/custom-lint.yaml', '${file}'],
        )

    def test_cmd_ignores_a_blank_config_path(self):
        self.linter.settings = {'executable': '/tools/cli', 'config_path': '   '}

        self.assertEqual(self.linter.cmd(), ['/tools/cli', '--json', '${file}'])

    def test_cmd_uses_configured_executable(self):
        self.linter.settings = {'executable': '/tools/PapyrusLinter'}

        self.assertEqual(
            self.linter.cmd(), ['/tools/PapyrusLinter', '--json', '${file}']
        )

    def test_cmd_downloads_matching_cli_when_executable_is_not_configured(self):
        self.linter.settings = {}
        with patch.object(
            self.module,
            'ensure_release_cli',
            return_value='/tmp/sublime-cache/PapyrusLinterCLI-linux',
        ) as ensure_cli:
            command = self.linter.cmd()

        self.assertEqual(
            command,
            [
                '/tmp/sublime-cache/PapyrusLinterCLI-linux',
                '--json',
                '${file}',
            ],
        )
        ensure_cli.assert_called_once_with('/tmp/sublime-cache')

    def test_find_errors_converts_every_diagnostic(self):
        report = {
            'files': [
                {
                    'path': 'first.psc',
                    'diagnostics': [
                        {
                            'line': 3,
                            'column': 7,
                            'level': 'warning',
                            'rule': 'slow-function',
                            'message': '[warning] Prefer an event.',
                        },
                        {
                            'line': 1,
                            'column': 1,
                            'level': 'info',
                            'rule': 'style',
                            'message': '[info] Style note.',
                        },
                    ],
                },
                {
                    'path': 'second.psc',
                    'diagnostics': [
                        {
                            'line': 9,
                            'column': 2,
                            'level': 'error',
                            'rule': 'parse',
                            'message': '[error] Invalid expression.',
                        }
                    ],
                },
            ]
        }

        matches = list(self.linter.find_errors(json.dumps(report)))

        self.assertEqual(len(matches), 3)
        self.assertEqual(
            vars(matches[0]),
            {
                'line': 2,
                'col': 6,
                'code': 'slow-function',
                'error_type': 'warning',
                'message': 'Prefer an event.',
            },
        )
        self.assertEqual(matches[1].error_type, 'warning')
        self.assertEqual(matches[1].message, 'Style note.')
        self.assertEqual(matches[2].error_type, 'error')
        self.assertEqual(matches[2].message, 'Invalid expression.')

    def test_find_errors_accepts_reports_without_files(self):
        self.assertEqual(list(self.linter.find_errors('{}')), [])

    def test_find_errors_reports_invalid_json_as_permanent_failure(self):
        with self.assertRaisesRegex(FakePermanentError, 'invalid JSON output'):
            list(self.linter.find_errors('not JSON'))

        self.linter.notify_failure.assert_called_once_with()
        self.linter.logger.error.assert_called_once()
        self.assertIn('not JSON', self.linter.logger.error.call_args.args[0])

    def test_message_without_level_tag_is_preserved(self):
        match = self.linter._to_lint_match(
            {'line': 1, 'column': 1, 'message': 'Plain diagnostic'}
        )

        self.assertEqual(match.error_type, 'error')
        self.assertEqual(match.message, 'Plain diagnostic')
        self.assertIsNone(match.code)


if __name__ == '__main__':
    unittest.main()

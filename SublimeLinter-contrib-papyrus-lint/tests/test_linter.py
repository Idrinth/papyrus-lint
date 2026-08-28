"""Tests for the SublimeLinter adapter without requiring Sublime Text."""

import importlib.util
import json
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import Mock


PLUGIN_ROOT = Path(__file__).resolve().parents[1]


class FakeLintMatch:
    """Small stand-in that exposes the fields supplied to LintMatch."""

    def __init__(self, **kwargs):
        self.__dict__.update(kwargs)


class FakeLinter:
    def __init__(self):
        self.logger = Mock()
        self.notify_failure = Mock()
        self.name = 'papyrus-lint'


class FakePermanentError(Exception):
    pass


def load_linter_module():
    lint_module = types.ModuleType('SublimeLinter.lint')
    lint_module.Linter = FakeLinter
    lint_module.LintMatch = FakeLintMatch
    lint_module.PermanentError = FakePermanentError

    package = types.ModuleType('SublimeLinter')
    package.lint = lint_module

    with unittest.mock.patch.dict(
        sys.modules,
        {'SublimeLinter': package, 'SublimeLinter.lint': lint_module},
    ):
        spec = importlib.util.spec_from_file_location(
            'papyrus_lint_sublime_linter', PLUGIN_ROOT / 'linter.py'
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
        self.assertEqual(
            self.linter.cmd, ('PapyrusLinterCLI', '--json', '${file}')
        )
        self.assertEqual(self.linter.executable, 'PapyrusLinterCLI')
        self.assertEqual(self.linter.defaults['selector'], 'source.papyrus')

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

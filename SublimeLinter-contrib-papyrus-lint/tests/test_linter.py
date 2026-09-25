"""Tests for the SublimeLinter adapter without requiring Sublime Text."""

import importlib.util
import json
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

PLUGIN_ROOT = Path(__file__).resolve().parents[1]


class FakeLintMatch:
    """Small stand-in that exposes the fields supplied to LintMatch."""

    def __init__(self, **kwargs):
        self.__dict__.update(kwargs)


class FakeRegion:
    """Stand-in for `sublime.Region`, only ever spanning the whole buffer here."""

    def __init__(self, a, b):
        self.a = a
        self.b = b


class FakeView:
    """Stand-in for `sublime.View`, exposing just what `cmd()` reads off it."""

    def __init__(self, text='', dirty=False):
        self._text = text
        self._dirty = dirty

    def is_dirty(self):
        return self._dirty

    def size(self):
        return len(self._text)

    def substr(self, region):
        return self._text[region.a:region.b]


class FakeLinter:
    def __init__(self, settings=None, view=None):
        self.logger = Mock()
        self.notify_failure = Mock()
        self.name = 'papyrus-lint'
        self.settings = settings if settings is not None else {}
        self.view = view if view is not None else FakeView()


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
    sublime.load_resource = Mock(side_effect=FileNotFoundError)
    sublime.Region = FakeRegion

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
        self.verify_patcher = patch.object(self.module, 'verify_configured_cli')
        self.verify_cli = self.verify_patcher.start()
        self.addCleanup(self.verify_patcher.stop)

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
            ['/tools/PapyrusLinterCLI', 'lint', '--format', 'json', '--config', '/project/custom-lint.yaml', '${file}'],
        )

    def test_cmd_ignores_a_blank_config_path(self):
        self.linter.settings = {'executable': '/tools/cli', 'config_path': '   '}

        self.assertEqual(self.linter.cmd(), ['/tools/cli', 'lint', '--format', 'json', '${file}'])

    def test_cmd_uses_configured_executable(self):
        self.linter.settings = {'executable': '/tools/PapyrusLinter'}

        self.assertEqual(
            self.linter.cmd(), ['/tools/PapyrusLinter', 'lint', '--format', 'json', '${file}']
        )

    def test_cmd_rejects_configured_executable_from_a_different_release(self):
        self.linter.settings = {'executable': '/tools/PapyrusLinterCLI'}
        self.verify_cli.side_effect = OSError('configured CLI version mismatch')

        with self.assertRaisesRegex(FakePermanentError, 'version mismatch'):
            self.linter.cmd()

        self.verify_cli.assert_called_once_with('/tools/PapyrusLinterCLI')

    def test_cmd_passes_the_buffer_as_a_blob_when_the_view_has_unsaved_changes(self):
        self.linter.settings = {'executable': '/tools/PapyrusLinterCLI'}
        self.linter.view = FakeView(text='ScriptName Test\n', dirty=True)

        self.assertEqual(
            self.linter.cmd(),
            ['/tools/PapyrusLinterCLI', 'lint', '--format', 'json', '--blob', 'ScriptName Test\n'],
        )

    def test_cmd_still_inserts_config_flag_before_a_blob(self):
        self.linter.settings = {
            'executable': '/tools/PapyrusLinterCLI',
            'config_path': '/project/custom-lint.yaml',
        }
        self.linter.view = FakeView(text='ScriptName Test\n', dirty=True)

        self.assertEqual(
            self.linter.cmd(),
            [
                '/tools/PapyrusLinterCLI',
                'lint', '--format', 'json',
                '--config',
                '/project/custom-lint.yaml',
                '--blob',
                'ScriptName Test\n',
            ],
        )

    def test_cmd_uses_the_real_file_once_the_view_is_saved(self):
        self.linter.settings = {'executable': '/tools/PapyrusLinterCLI'}
        self.linter.view = FakeView(text='ScriptName Test\n', dirty=False)

        self.assertEqual(
            self.linter.cmd(),
            ['/tools/PapyrusLinterCLI', 'lint', '--format', 'json', '${file}'],
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
                'lint', '--format', 'json',
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

    def test_doc_url_is_appended_to_the_message_when_present(self):
        match = self.linter._to_lint_match(
            {
                'line': 1,
                'column': 1,
                'rule': 'trailing-whitespace',
                'message': '[warning] Trailing whitespace.',
                'doc_url': 'https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace',
            }
        )

        self.assertEqual(
            match.message,
            'Trailing whitespace. (see: https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace)',
        )

    def test_a_null_doc_url_leaves_the_message_untouched(self):
        match = self.linter._to_lint_match(
            {
                'line': 1,
                'column': 1,
                'rule': 'compiler-error',
                'message': '[error] syntax error',
                'doc_url': None,
            }
        )

        self.assertEqual(match.message, 'syntax error')


if __name__ == '__main__':
    unittest.main()

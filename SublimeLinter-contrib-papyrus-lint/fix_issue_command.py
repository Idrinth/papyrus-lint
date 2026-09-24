"""Command that applies the PapyrusLint fix nearest the caret."""

import json
import subprocess

import sublime
import sublime_plugin

from .command_helpers import PapyrusLintCliSettings, windows_startupinfo


class PapyrusLintFixIssueCommand(PapyrusLintCliSettings, sublime_plugin.TextCommand):
    """Fix the single diagnostic under the caret in the current saved file."""

    def is_visible(self):
        file_name = self.view.file_name()
        return bool(file_name) and file_name.lower().endswith('.psc')

    def is_enabled(self):
        return bool(self.view.file_name()) and not self.view.is_dirty()

    def run(self, edit):
        file_name = self.view.file_name()
        if not file_name:
            return
        selection = self.view.sel()
        if not selection:
            return
        row, col = self.view.rowcol(selection[0].begin())
        target_line, target_column = row + 1, col + 1

        try:
            executable = self._executable()
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return

        startupinfo = windows_startupinfo()
        report = self._run_cli(
            executable, ['--json'], file_name, startupinfo, 'read diagnostics'
        )
        if report is None:
            return
        diagnostic = self._diagnostic_near(report, target_line, target_column)
        if diagnostic is None:
            sublime.error_message('PapyrusLint: no issue reported on the current line.')
            return

        fix_args = ['fix']
        rule = diagnostic.get('rule')
        if rule:
            fix_args += ['--type', rule]
        fix_args += ['--line', str(target_line)]
        if self._run_cli(executable, fix_args, file_name, startupinfo, 'fix') is None:
            return
        self.view.run_command('revert')
        self.view.run_command('sublime_linter_lint')

    def _run_cli(self, executable, args, file_name, startupinfo, failure_label):
        command = [executable] + args
        config_path = self._config_path()
        if config_path:
            command += ['--config', config_path]
        command.append(file_name)
        try:
            result = subprocess.run(
                tuple(command), capture_output=True, startupinfo=startupinfo
            )
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return None
        if result.returncode not in (0, 1):
            message = result.stderr.decode('utf-8', 'replace').strip()
            sublime.error_message(
                'PapyrusLint: {} failed:\n{}'.format(
                    failure_label, message or 'unknown error'
                )
            )
            return None
        return result.stdout

    @staticmethod
    def _diagnostic_near(report_bytes, target_line, target_column):
        """Return the diagnostic on the target line nearest the target column."""
        try:
            report = json.loads(report_bytes)
        except ValueError:
            return None
        candidates = [
            diagnostic
            for file_report in report.get('files', [])
            for diagnostic in file_report.get('diagnostics', [])
            if diagnostic.get('line') == target_line
        ]
        if not candidates:
            return None
        return min(
            candidates,
            key=lambda diagnostic: abs(
                diagnostic.get('column', target_column) - target_column
            ),
        )

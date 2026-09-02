"""This module exports the PapyrusLint fix commands."""

import json
import os
import subprocess

import sublime
import sublime_plugin

from .cli_download import ensure_release_cli


def _windows_startupinfo():
    """Builds a `STARTUPINFO` that hides the CLI's console window on Windows.

    Returns `None` on every other platform, where `subprocess.run` ignores
    the argument anyway.
    """
    if os.name != 'nt':
        return None
    startupinfo = subprocess.STARTUPINFO()
    startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    return startupinfo


class _PapyrusLintCliCommand(sublime_plugin.TextCommand):
    """Shared helpers for locating and invoking the configured CLI executable.

    Both `PapyrusLintFixCommand` and `PapyrusLintFixIssueCommand` need the
    same `executable`/`config_path` linter settings, so that lookup lives
    here rather than being duplicated in each command.
    """

    def _executable(self):
        return self._linter_settings().get('executable') or ensure_release_cli(
            sublime.cache_path()
        )

    def _config_path(self):
        return (self._linter_settings().get('config_path') or '').strip()

    @staticmethod
    def _linter_settings():
        settings = sublime.load_settings('SublimeLinter.sublime-settings')
        return settings.get('linters', {}).get('papyrus-lint', {})


class PapyrusLintFixCommand(_PapyrusLintCliCommand):
    """Runs `PapyrusLinterCLI fix` against the current file, then reloads it.

    Mirrors the desktop app's "Fix" button: PapyrusLinterCLI's `fix`
    subcommand applies every automatic fix (see the main project's
    README.md) to the file on disk, rewriting it if anything changed,
    before reporting whatever diagnostics remain. Like the linter itself,
    this reads/writes the file straight off disk rather than the Sublime
    buffer, so it only runs against a saved file with no unsaved changes.
    """

    def is_visible(self):
        file_name = self.view.file_name()
        return bool(file_name) and file_name.lower().endswith('.psc')

    def is_enabled(self):
        return bool(self.view.file_name()) and not self.view.is_dirty()

    def run(self, edit):
        file_name = self.view.file_name()
        if not file_name:
            return

        startupinfo = _windows_startupinfo()

        try:
            executable = self._executable()
            command = [executable, 'fix']
            config_path = self._config_path()
            if config_path:
                command += ['--config', config_path]
            command.append(file_name)
            result = subprocess.run(
                tuple(command),
                capture_output=True,
                startupinfo=startupinfo,
            )
        except OSError as err:
            sublime.error_message(
                'PapyrusLint: failed to download or run the CLI: {}'.format(err)
            )
            return

        # Exit status 0 (clean) and 1 (diagnostics remain after fixing) are
        # both a normal outcome here; only 2 (usage/I/O error) is a failure.
        if result.returncode not in (0, 1):
            message = result.stderr.decode('utf-8', 'replace').strip()
            sublime.error_message(
                'PapyrusLint: fix failed:\n{}'.format(message or 'unknown error')
            )
            return

        self.view.run_command('revert')
        self.view.run_command('sublime_linter_lint')


class PapyrusLintFixIssueCommand(_PapyrusLintCliCommand):
    """Fixes only the single diagnostic under the caret, then reloads the file.

    Unlike `PapyrusLintFixCommand`, which applies every enabled automatic
    fix across the whole file, this narrows `PapyrusLinterCLI fix` to the
    one diagnostic nearest the caret's line/column via its `--type` and
    `--line` flags (see the main project's README.md), leaving every other
    line and every other rule's findings untouched.

    Diagnostics are re-read fresh via `PapyrusLinterCLI --json` rather than
    reused from SublimeLinter's own last lint pass, so this always acts on
    up-to-date output for the file currently on disk. As with
    `PapyrusLintFixCommand`, this only runs against a saved file with no
    unsaved changes.
    """

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
        target_line = row + 1
        target_column = col + 1

        startupinfo = _windows_startupinfo()

        try:
            executable = self._executable()
        except OSError as err:
            sublime.error_message(
                'PapyrusLint: failed to download or run the CLI: {}'.format(err)
            )
            return

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

        # Exit status 0 (clean) and 1 (diagnostics remain after fixing) are
        # both a normal outcome here; only 2 (usage/I/O error) is a failure.
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
                tuple(command),
                capture_output=True,
                startupinfo=startupinfo,
            )
        except OSError as err:
            sublime.error_message(
                'PapyrusLint: failed to download or run the CLI: {}'.format(err)
            )
            return None

        if result.returncode not in (0, 1):
            message = result.stderr.decode('utf-8', 'replace').strip()
            sublime.error_message(
                'PapyrusLint: {} failed:\n{}'.format(failure_label, message or 'unknown error')
            )
            return None

        return result.stdout

    @staticmethod
    def _diagnostic_near(report_bytes, target_line, target_column):
        """Picks the diagnostic on `target_line` closest to `target_column`.

        Returns `None` if `report_bytes` isn't valid JSON or no diagnostic
        was reported on that line at all.
        """
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
            key=lambda diagnostic: abs(diagnostic.get('column', target_column) - target_column),
        )

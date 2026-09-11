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


class _PapyrusLintCliSettings:
    """Shared helpers for locating the configured CLI executable/config path.

    Mixed into both `_PapyrusLintCliCommand` (a `TextCommand`, for the
    file-scoped fix commands below) and `PapyrusLintInitCommand` (a
    `WindowCommand`, since `init` isn't scoped to any one file), so this
    lookup lives in one place rather than being duplicated for each base
    class.
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


class _PapyrusLintCliCommand(_PapyrusLintCliSettings, sublime_plugin.TextCommand):
    """Shared helpers for locating and invoking the configured CLI executable.

    Both `PapyrusLintFixCommand` and `PapyrusLintFixIssueCommand` need the
    same `executable`/`config_path` linter settings, so that lookup lives
    in `_PapyrusLintCliSettings` rather than being duplicated in each
    command.
    """


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
                f'PapyrusLint: failed to download or run the CLI: {err}'
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
                f'PapyrusLint: failed to download or run the CLI: {err}'
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
                f'PapyrusLint: failed to download or run the CLI: {err}'
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


class PapyrusLintInitCommand(_PapyrusLintCliSettings, sublime_plugin.WindowCommand):
    """Runs `PapyrusLinterCLI init [--preset <name>]` to scaffold a config.

    Unlike the fix commands above, this isn't scoped to any one file, so
    it's a `WindowCommand` (available from the Command Palette only, not
    the editor/explorer context menus) rather than a `TextCommand`. It
    picks a target directory to run `init` in from the window's open
    folder(s) - prompting when more than one is open - falling back to the
    active view's own directory when no folder is open at all; then
    prompts for the `--preset` to pass, mirroring the CLI's own built-in
    choices (`strict`, `standard`, `careful`) plus a free-form entry for a
    custom preset added via `PapyrusLinterCLI preset add` or the desktop
    app's "Save current settings as preset..." button. Never overwrites an
    existing papyrus-lint.yaml/.yml, the same as the CLI itself.
    """

    #: Shown in the preset quick panel, in the same order as the CLI's own
    #: `--preset` documentation; the last entry opens a follow-up input
    #: panel for a custom preset name instead of picking one of these.
    PRESET_LABELS = ['strict (default)', 'standard', 'careful', 'Custom preset name…']

    #: The `--preset` value for each of `PRESET_LABELS`' built-in entries,
    #: by index; `None` omits `--preset` entirely (the CLI's own "strict"
    #: default). Has no entry for the trailing "Custom preset name…" label,
    #: which is handled separately via `_on_custom_preset_entered`.
    PRESET_VALUES = [None, 'standard', 'careful']

    def run(self):
        folders = self.window.folders()
        if len(folders) == 1:
            self._start(folders[0])
            return
        if len(folders) > 1:
            self.window.show_quick_panel(
                folders, lambda index: self._on_folder_chosen(folders, index)
            )
            return

        view = self.window.active_view()
        file_name = view.file_name() if view else None
        if not file_name:
            sublime.error_message('PapyrusLint: open a folder or a saved file first.')
            return
        self._start(os.path.dirname(file_name))

    def _on_folder_chosen(self, folders, index):
        if index == -1:
            return
        self._start(folders[index])

    def _start(self, directory):
        self._directory = directory
        self.window.show_quick_panel(self.PRESET_LABELS, self._on_preset_chosen)

    def _on_preset_chosen(self, index):
        if index == -1:
            return
        if index == len(self.PRESET_LABELS) - 1:
            self.window.show_input_panel(
                'Custom preset name:', '', self._on_custom_preset_entered, None, None
            )
            return
        self._run_init(self.PRESET_VALUES[index])

    def _on_custom_preset_entered(self, name):
        name = name.strip()
        if not name:
            return
        self._run_init(name)

    def _run_init(self, preset):
        startupinfo = _windows_startupinfo()

        try:
            executable = self._executable()
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return

        command = [executable, 'init']
        if preset:
            command += ['--preset', preset]

        try:
            result = subprocess.run(
                tuple(command),
                capture_output=True,
                cwd=self._directory,
                startupinfo=startupinfo,
            )
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return

        if result.returncode != 0:
            message = result.stderr.decode('utf-8', 'replace').strip()
            sublime.error_message(
                'PapyrusLint: init failed:\n{}'.format(message or 'unknown error')
            )
            return

        sublime.status_message(result.stdout.decode('utf-8', 'replace').strip())

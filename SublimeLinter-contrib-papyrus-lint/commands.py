"""This module exports the PapyrusLint fix command."""

import os
import subprocess

import sublime
import sublime_plugin


class PapyrusLintFixCommand(sublime_plugin.TextCommand):
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

        executable = self._executable()
        startupinfo = None
        if os.name == 'nt':
            startupinfo = subprocess.STARTUPINFO()
            startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW

        try:
            result = subprocess.run(
                (executable, 'fix', file_name),
                capture_output=True,
                startupinfo=startupinfo,
            )
        except OSError as err:
            sublime.error_message(
                'PapyrusLint: failed to run "{}": {}'.format(executable, err)
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

    def _executable(self):
        settings = sublime.load_settings('SublimeLinter.sublime-settings')
        linter_settings = settings.get('linters', {}).get('papyrus-lint', {})
        return linter_settings.get('executable') or 'PapyrusLinterCLI'

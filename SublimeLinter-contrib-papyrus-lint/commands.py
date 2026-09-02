"""This module exports the PapyrusLint fix command."""

import os
import subprocess

import sublime
import sublime_plugin

from .cli_download import ensure_release_cli


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

        startupinfo = None
        if os.name == 'nt':
            startupinfo = subprocess.STARTUPINFO()
            startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW

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

"""Command that applies all available PapyrusLint fixes."""

import subprocess

import sublime
import sublime_plugin

from .command_helpers import PapyrusLintCliSettings, windows_startupinfo


class PapyrusLintFixCommand(PapyrusLintCliSettings, sublime_plugin.TextCommand):
    """Run ``PapyrusLinterCLI fix`` against the current saved file."""

    def is_visible(self):
        file_name = self.view.file_name()
        return bool(file_name) and file_name.lower().endswith('.psc')

    def is_enabled(self):
        return bool(self.view.file_name()) and not self.view.is_dirty()

    def run(self, edit):
        file_name = self.view.file_name()
        if not file_name:
            return

        try:
            executable = self._executable()
            command = [executable, 'fix']
            config_path = self._config_path()
            if config_path:
                command += ['--config', config_path]
            command.append(file_name)
            result = subprocess.run(
                tuple(command), capture_output=True, startupinfo=windows_startupinfo()
            )
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return

        if result.returncode not in (0, 1):
            message = result.stderr.decode('utf-8', 'replace').strip()
            sublime.error_message(
                'PapyrusLint: fix failed:\n{}'.format(message or 'unknown error')
            )
            return

        self.view.run_command('revert')
        self.view.run_command('sublime_linter_lint')

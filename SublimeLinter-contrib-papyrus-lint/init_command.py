"""Command that creates a PapyrusLint configuration."""

import os
import subprocess

import sublime
import sublime_plugin

from .command_helpers import PapyrusLintCliSettings, windows_startupinfo


class PapyrusLintInitCommand(PapyrusLintCliSettings, sublime_plugin.WindowCommand):
    """Run ``PapyrusLinterCLI init`` for a folder selected by the user."""

    PRESET_LABELS = ['strict (default)', 'standard', 'careful', 'Custom preset name…']
    PRESET_VALUES = [None, 'standard', 'careful']
    GAME_LABELS = ['Skyrim (default)', 'Fallout 4']
    GAME_VALUES = ['skyrim', 'fallout4']

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
        if index != -1:
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
        self._ask_for_game(self.PRESET_VALUES[index])

    def _on_custom_preset_entered(self, name):
        name = name.strip()
        if name:
            self._ask_for_game(name)

    def _ask_for_game(self, preset):
        self._preset = preset
        self.window.show_quick_panel(self.GAME_LABELS, self._on_game_chosen)

    def _on_game_chosen(self, index):
        if index != -1:
            self._run_init(self._preset, self.GAME_VALUES[index])

    def _run_init(self, preset, game):
        try:
            executable = self._executable()
        except OSError as err:
            sublime.error_message(
                f'PapyrusLint: failed to download or run the CLI: {err}'
            )
            return
        command = [executable, 'init', '--game', game]
        if preset:
            command += ['--preset', preset]
        try:
            result = subprocess.run(
                tuple(command),
                capture_output=True,
                cwd=self._directory,
                startupinfo=windows_startupinfo(),
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

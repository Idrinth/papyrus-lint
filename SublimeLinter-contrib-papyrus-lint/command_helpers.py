"""Shared helpers for PapyrusLint Sublime Text commands."""

import os
import subprocess

import sublime

from .cli_download import ensure_release_cli, verify_configured_cli


def windows_startupinfo():
    """Build a startup configuration that hides the CLI window on Windows."""
    if os.name != 'nt':
        return None
    startupinfo = subprocess.STARTUPINFO()
    startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    return startupinfo


class PapyrusLintCliSettings:
    """Locate the CLI executable and optional configuration path."""

    def _executable(self):
        executable = self._linter_settings().get('executable')
        if executable:
            verify_configured_cli(executable)
            return executable
        return ensure_release_cli(sublime.cache_path())

    def _config_path(self):
        return (self._linter_settings().get('config_path') or '').strip()

    @staticmethod
    def _linter_settings():
        settings = sublime.load_settings('SublimeLinter.sublime-settings')
        return settings.get('linters', {}).get('papyrus-lint', {})

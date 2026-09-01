"""This module exports the PapyrusLint plugin class."""

import json
import re

from SublimeLinter.lint import Linter, LintMatch, PermanentError

# Strips a diagnostic message's leading "[error]"/"[warning]"/"[info]" tag
# (see app/crates/papyrus-lints/src/lib.rs Diagnostic::level) before it's shown
# in Sublime, since `level` is already surfaced as its own JSON field.
_LEVEL_TAG = re.compile(r'^\[(?:error|warning|info)\]\s*')


class PapyrusLint(Linter):
    """Runs a Papyrus Lint executable against a `.psc` file.

    `PapyrusLinterCLI` (from https://github.com/Idrinth/papyrus-lint) reads
    the file straight off disk rather than accepting it on stdin, and uses
    its containing directory to look up an optional `papyrus-lint.yaml`/
    `.yml` configuration next to it, so this linter only checks a script
    once it's saved (there's no `on_stdin` here). It's run with `--json`
    so diagnostics are parsed from PapyrusLinterCLI's structured report
    (see `JsonReport` in app/crates/papyrus-lint-cli/src/lib.rs) instead of
    scraping its plain-text output. The `config_path` setting (see
    `defaults` below) passes `--config <path>` to the CLI, overriding its
    project-root `papyrus-lint.yaml`/`.yml` discovery.
    """

    executable = 'PapyrusLinterCLI'

    defaults = {
        'selector': 'source.papyrus',
        # An explicit papyrus-lint config file path, passed to the CLI via
        # `--config` when set. Empty (the default) leaves the CLI to
        # discover papyrus-lint.yaml/.yml from the project root as usual.
        'config_path': '',
    }

    def cmd(self):
        """Builds the command, inserting `--config <path>` when configured.

        `config_path` (see `defaults` above) is this linter's own setting,
        not one of SublimeLinter's built-ins, so it's read directly from
        `self.settings` here rather than via a `${...}` substitution in a
        static `cmd` tuple, which would leave a stray empty argument when
        unset.
        """
        executable = self.settings.get('executable') or self.executable
        command = [executable, '--json']
        config_path = (self.settings.get('config_path') or '').strip()
        if config_path:
            command += ['--config', config_path]
        command.append('${file}')
        return command

    def find_errors(self, output):
        """Parse `PapyrusLinterCLI --json`'s report instead of a regex.

        `output` is the single JSON document PapyrusLinterCLI prints to
        stdout: a `{"files": [{"path", "diagnostics": [...]}], ...}`
        report (see `JsonReport`/`JsonFileReport`/`JsonDiagnostic` in
        app/crates/papyrus-lint-cli/src/lib.rs). Since this linter always
        invokes PapyrusLinterCLI with a single `.psc` file argument, that
        report only ever contains one file entry; every diagnostic across
        every entry is yielded regardless, so this still works if that
        ever changes.
        """
        try:
            report = json.loads(output)
        except ValueError:
            self.logger.error(
                '{}: could not parse JSON output:\n{}'.format(self.name, output)
            )
            self.notify_failure()
            raise PermanentError('invalid JSON output')

        for file_report in report.get('files', []):
            for diagnostic in file_report.get('diagnostics', []):
                yield self._to_lint_match(diagnostic)

    @staticmethod
    def _to_lint_match(diagnostic):
        """Converts one `JsonDiagnostic` object into a `LintMatch`.

        Line/column are converted from PapyrusLinterCLI's 1-based
        coordinates to the 0-based ones `process_match` expects (normally
        `split_match` does this via `line_col_base`, but that only runs
        for the regex-based default `find_errors`, which this bypasses).
        """
        level = diagnostic.get('level')
        return LintMatch(
            line=diagnostic['line'] - 1,
            col=diagnostic['column'] - 1,
            code=diagnostic.get('rule'),
            error_type='warning' if level in ('warning', 'info') else 'error',
            message=_LEVEL_TAG.sub('', diagnostic.get('message', ''), count=1),
        )

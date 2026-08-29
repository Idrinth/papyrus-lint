"""This module exports the PapyrusLint plugin class."""

import json
import re

from SublimeLinter.lint import Linter, LintMatch, PermanentError

# Strips a diagnostic message's leading "[error]"/"[warning]"/"[info]" tag
# (see app/crates/papyrus-lints/src/lib.rs Diagnostic::level) before it's shown
# in Sublime, since `level` is already surfaced as its own JSON field.
_LEVEL_TAG = re.compile(r'^\[(?:error|warning|info)\]\s*')


class PapyrusLint(Linter):
    """Runs the standalone `PapyrusLinterCLI` binary against a `.psc` file.

    `PapyrusLinterCLI` (from https://github.com/Idrinth/papyrus-lint) reads
    the file straight off disk rather than accepting it on stdin, and uses
    its containing directory to look up an optional `papyrus-lint.yaml`/
    `.yml` configuration next to it, so this linter only checks a script
    once it's saved (there's no `on_stdin` here). It's run with `--json`
    so diagnostics are parsed from PapyrusLinterCLI's structured report
    (see `JsonReport` in app/crates/papyrus-lint-cli/src/lib.rs) instead of
    scraping its plain-text output.
    """

    cmd = ('PapyrusLinterCLI', '--json', '${file}')
    executable = 'PapyrusLinterCLI'

    defaults = {
        'selector': 'source.papyrus',
    }

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

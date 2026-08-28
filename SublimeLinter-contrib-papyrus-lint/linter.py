"""This module exports the PapyrusLint plugin class."""

from SublimeLinter.lint import Linter


class PapyrusLint(Linter):
    """Runs the standalone `PapyrusLinterCLI` binary against a `.psc` file.

    `PapyrusLinterCLI` (from https://github.com/Idrinth/papyrus-lint) reads
    the file straight off disk rather than accepting it on stdin, and uses
    its containing directory to look up an optional `papyrus-lint.yaml`/
    `.yml` configuration next to it, so this linter only checks a script
    once it's saved (there's no `on_stdin` here).
    """

    cmd = ('PapyrusLinterCLI', '${file}')
    executable = 'PapyrusLinterCLI'

    # PapyrusLinterCLI prints one line per diagnostic as:
    #   <path>:<line>:<column>: [<rule>] <message>
    # where <message> may itself start with a "[error]"/"[warning]"/"[info]"
    # severity tag; a diagnostic with no tag at all is always an error (see
    # crates/papyrus-lints/src/lib.rs `Diagnostic::level`).
    regex = (
        r'^.+?:(?P<line>\d+):(?P<col>\d+): '
        r'\[(?P<code>[\w-]+)\] '
        r'(?:\[(?:(?P<error>error)|(?P<warning>warning)|(?P<info>info))\]\s*)?'
        r'(?P<message>.+)$'
    )

    multiline = False
    line_col_base = (1, 1)
    default_type = 'error'

    defaults = {
        'selector': 'source.papyrus',
    }

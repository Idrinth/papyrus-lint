# Compiling a script from the desktop app

Each `.psc` file listed on the Lint results tab has a "Compile" button that
recompiles it with `PapyrusCompiler.exe` (see `compiler_path` in the
[configuration reference](configuration.md) for how that executable's path is resolved), so a fix
made in the code viewer can be tried out without leaving the app. The code
viewer's editor has the same capability built in: alongside "Save" and
"Cancel", a "Save & Compile" button writes the edited script to disk and
then immediately recompiles it, showing the result beneath the editor —
so a fix can be saved and verified in one step, without reopening the
file from the Lint results list. Either button runs:

```text
PapyrusCompiler.exe "<script path>" -i="<source dir 1>;<source dir 2>" -o="<output dir>" -f="TESV_Papyrus_Flags.flg"
```

where `<script path>` is the path to the `.psc` file, whose parent directory
is the source directory
(conventionally a `scripts/source` or `source/scripts` directory under the
project root) and `<output dir>` is its parent, matching the layout
Bethesda's tooling expects — a `Source` directory holding `.psc` files
inside the `Scripts` directory that receives the compiled `.pex` output.
`-i` is given both of those conventional source directories under the
project root, plus any configured `additional_script_roots` (see the
configuration reference), separated by `;` (PapyrusCompiler.exe accepts
multiple import directories that way), so the script can still resolve
imports from the other layout, or a configured additional root, even
though it only lives in one of them.
The compiler is run with its containing directory as the working directory,
allowing it to resolve the bundled `TESV_Papyrus_Flags.flg` passed by the final
`-f` argument.

The compiler's stdout/stderr is shown once it finishes (beneath the
"Compile" button on the Lint results list, or beneath the editor for
"Save & Compile"), styled green on success and red on failure, so both a
successful compile and a reported error (a syntax error, a missing
import, etc.) are visible without checking a log file. If no compiler
path is configured or auto-detected, or the executable itself can't be
run, that's reported the same way rather than silently doing nothing.

`PapyrusCompiler.exe` embeds the compiling machine's Windows username and
computer name into every `.pex` it writes, right next to the source file
name in its header. On a successful compile, Papyrus Lint reads that
header back out of the resulting `.pex` and blanks both fields in place,
so a script compiled locally and then shared (e.g. bundled into a mod)
doesn't leak who built it or what machine they built it on. A note is
added to the compile output when this happens.

Enabling `compile_check` (see the configuration reference) also runs
PapyrusCompiler.exe as part of linting a `.psc` — automatically, not just
from the "Compile"/"Save & Compile" buttons — and reports any errors it
finds as `[error]` diagnostics alongside the lint engine's own, so a
syntax mistake the compiler itself rejects (but the lint engine's own,
more forgiving parser doesn't) still shows up in the results. Unlike the
"Compile" button above, this always compiles into a throwaway temporary
directory rather than the project's real `Scripts` output directory, so
it never overwrites (or requires write access to) the project's actual
compiled `.pex` output, and never needs the personal-data stripping
described above — the compiled output is discarded either way. The CLI
honors the same setting during a normal lint/fix run (not just its own
`doctor` subcommand's validation of it), reading `compile_check` and
`compiler_path` from the resolved project's `papyrus-lint.yaml`/`.yml`.

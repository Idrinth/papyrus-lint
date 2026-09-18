# Command-line interface reference

Besides its GUI, Papyrus Lint can lint non-interactively from the
command line two ways: by passing an `.achlist` (or a single `.psc`, or a
directory) path to the desktop app's own executable (`PapyrusLinter`), or
via the standalone `PapyrusLinterCLI` binary (`app/crates/papyrus-lint-cli`)
built and shipped separately for use cases — e.g. a CI pipeline — that
shouldn't need the desktop app's binary (and its GUI dependencies) at all.
Both accept the same argument and behave identically:

[See the docs](papyrus-cli-usage.txt) for a list of possible arguments
and options or read on for explanations.

## Initializing a project (`init`, `preset add`, `doctor`)

`PapyrusLinterCLI init` creates a `papyrus-lint.yaml` in the current working
directory from the selected `--preset` (`strict`, `standard`, or `careful`,
matched case-insensitively; defaults to `strict`, identical to today's
built-in default — see the [configuration reference](configuration.md)).
Any other `--preset` name is looked up
as `<name>.yaml`/`.yml` (matched case-insensitively) in a `presets`
directory next to the running executable (see the configuration
reference). It
refuses to overwrite an existing `papyrus-lint.yaml` or `papyrus-lint.yml`
file, and a `--preset` name matching neither a built-in nor a file in that
directory is reported as an error.

If a `papyrus-lint.yaml`/`.yml` file exists next to the running executable
(the CLI binary itself, or the desktop app's binary when it delegates to CLI
mode), `init` merges it in as the base instead of the selected preset's own
settings: any key it sets overrides the preset, and any key it omits still
falls back to the preset. This lets you define your own baseline settings
once, next to wherever you keep the binary, and reuse it across every
project you run `init` in — on top of whichever preset you pick each time —
instead of hand-editing each newly generated file the same way.

`PapyrusLinterCLI preset add <name> <path>` saves an existing
`papyrus-lint.yaml`/`.yml` as a user preset named `<name>`, so it becomes
selectable via `init --preset <name>` afterward (see the configuration
reference). It refuses a blank name or one matching a built-in preset (`strict`,
`standard`, `careful`), and refuses to overwrite a preset that already
exists under that name unless `--yes` is also given.

`PapyrusLinterCLI doctor <path-to-achlist-or-psc-or-directory>` validates a
project's setup without linting any script: that the given path itself
exists (and, for an `.achlist`, that every entry it lists exists on disk);
that a discovered — or `--config`-overridden — `papyrus-lint.yaml`/`.yml`
actually parses; that at least one of `scripts/source`/`source/scripts`
exists under the resolved project root; that each configured
`additional_script_roots` entry (and any `--script-root` given alongside
`doctor`) and each configured `lookup_script_roots` entry resolves to an
existing directory; and that a configured, or
auto-detected, `compiler_path` points at an existing file — warning
instead if `compile_check` is enabled but no compiler path could be
resolved at all. Each check is printed as its own `[ok]`/`[warning]`/
`[error] <message>` line, or, with `--json`, as part of a single JSON
document (`{"project_root", "checks": [{"status", "message"}, ...],
"success"}`) instead. It exits `0` if every check passed, `1` if any
reported a `warning` or `error`, or `2` on a usage error — the same
convention every other subcommand follows.

## Resolving a project

Given an `.achlist` path, it resolves every `.psc` entry listed in it.
Given a single `.psc` path directly, it lints just that file, treating it
as the achlist's sole entry. Given a directory instead, it recursively
scans it (and every subdirectory beneath it, at any depth) for `.psc`
files and lints every one found — useful for a mod whose scripts are
spread across arbitrarily nested subfolders instead of a flat
`scripts/source` (e.g. Requiem's own layout) and that ships no `.achlist`
at all. Either way, each script is linted against the project's
`papyrus-lint.yaml`/`.yml` config file (see the configuration reference).
For an `.achlist`, the project root is its containing directory; for a bare
`.psc`, the root is found by walking up from the file for a
`Scripts/Source` or `Source/Scripts` directory pair (matched
case-insensitively) and taking the directory above it — so it's found
correctly even for a script nested further still, e.g. a namespaced
`Scripts/Source/User/MyScript.psc`, not just the conventional two
directories up. If no such pair exists in the path at all (e.g. a project
laid out like Requiem's own, without a `scripts/source` tree), it instead
looks for the nearest ancestor directory that already has a
`papyrus-lint.yaml`/`.yml`, so that project's config is still picked up for
a single file linted directly (e.g. by an editor plugin on save) — only
falling back to the fixed two-directories-up guess if neither finds
anything. A scanned directory's own resolved scripts are tried the
same way first, falling back to the scanned directory itself as the
project root if none of them match that layout. If no config exists
there, the documented defaults apply. Each diagnostic found is printed as
`<path>:<line>:<column>: [<rule>] <message>`, followed by that rule's own
documentation link on the [project website](https://papyrus-lint.idrinth.de)
when it has known tag metadata (a compiler-reported diagnostic doesn't),
then a one-line summary. Calls to functions declared on other scripts under the project
root are resolved the same way the desktop app resolves them, so the
CLI's "Argument type check"/"Return type check" results match what
dropping the same `.achlist` into the app would report.

Given `--config <path>` (combinable with `fix`/`--json`, in any argument
order), the CLI loads lint configuration directly from `<path>` instead
of discovering `papyrus-lint.yaml`/`.yml` from the project root — useful
when a config file lives somewhere other than that project root, or isn't
named `papyrus-lint.yaml`/`.yml`. Both editor plugins expose this as a
`config_path`/`configPath` setting (see their own READMEs). Since the
project root's own config file is bypassed entirely in that case, so is
its `additional_script_roots`; use `--script-root` (below) to add any
script roots back explicitly. `lookup_script_roots` and
`strict_achlist_scope` are still read from
`<path>` itself, the same as every other lint setting, since they aren't tied
to the project root the way `additional_script_roots` is.

Given one or more `--script-root <path>` flags (combinable with
`fix`/`--json`/`--config`, in any argument order), each given directory
(resolved relative to the project root unless already absolute) is
searched for `.psc` files alongside `scripts/source`/`source/scripts` and
the project's configured `additional_script_roots` (see the configuration
reference) — letting a caller add a script root for a single run without
editing the project's config file.

Given `--blob <source>` in place of a path argument, the CLI lints
`<source>` directly as raw Papyrus source text instead of resolving an
`.achlist`/`.psc`/directory from disk — useful for linting a script buffer
that isn't (yet, or ever) saved as a real file, e.g. from an editor
extension or another tool that already has the text in memory. The
diagnostics are reported under the literal path `<blob>`. There's no real
project behind a blob, so cross-script "Argument type check"/"Return type
check" resolution, the `conflicting_script_versions`/
`stale_compiled_output`/`script_filename_mismatch` project lints, and
`compile_check` don't apply — a call into another script is treated the
same as a call into an unknown one. `--config <path>` still selects an
explicit configuration file to lint the blob against; without it, the
engine's default configuration applies, since there's no project root to
discover one from. `--blob` is combinable with `--json`/`--format`/
`--hash-source`/`--quiet-warnings`/`--quiet-info`/`--tag`/`--color`/
`--output`, in any argument order, but it's a usage error alongside a path
argument, `fix`, `--type`, `--line`, `--dry-run`, `--script-root`,
`--progress`, or `--threads` — none of which mean anything without a real
file to resolve scripts around or write fixes back to.

## Output options

Given `--output <path>` (combinable with `fix`/`--json`/`--config`/
`--script-root`, in any argument order), the report — plain text or JSON,
whichever `--json` selects — is written to `<path>` instead of stdout, so
it can be stored directly without piping the command's output to a file.
Usage/error text still goes to stderr either way, and the exit status is
unaffected.

Given `--short-paths` (combinable with `fix`/`--json`/`--config`/
`--script-root`/`--output`, in any argument order), each script's path in
the report has the project root stripped from its beginning, the same way
the desktop app shortens paths in its own results list; a path that isn't
under the project root is left unchanged.

Given `--progress` (combinable with `fix`/`--json`/`--config`/
`--script-root`/`--short-paths`, in any argument order), a live
`<files linted>/<total files to lint>` progress bar is written to stdout as
each script finishes linting. This only makes sense alongside `--output
<path>`, since otherwise the report itself would also be writing to
stdout; using `--progress` without `--output` is a usage error (exit
status `2`) rather than mixing the two together on stdout.

Given `--color <auto|always|never>` (default `auto`, combinable with every
flag above), the plain-text report's diagnostic locations, rule tags, and
`[error]`/`[warning]`/`[info]` level tags are colorized with ANSI escapes,
and the summary line is colorized green/yellow/red for no problems/problems
that didn't fail the run/problems that did. `auto` colorizes only when
stdout is a real terminal, `--output` isn't used (a file is never a
terminal), and the `NO_COLOR` environment variable isn't set; `always`/
`never` override that detection outright. `--json` output is never
colorized, since it's meant for tooling rather than a terminal.

Given `--threads <n>` (combinable with every flag above), up to `<n>`
scripts are read, fixed, and linted at once instead of one at a time —
useful on a large `.achlist` or a directory scan with hundreds of scripts.
Defaults to the machine's available parallelism; `--threads 1` forces the
previous fully sequential behavior. The report is always assembled in the
scripts' original order regardless of thread count, so `--threads` never
changes what's reported — only how long it takes.

Each `.psc` file is decoded as UTF-8 when it's valid UTF-8, or as
Windows-1252 (CP1252) — the Creation Kit/Papyrus compiler's own default
encoding for the language — otherwise, so a script saved in either
encoding lints correctly instead of aborting the whole run.

## Applying fixes (`fix`)

Prefixed with the `fix` subcommand, it applies every automatic fix (the
rules marked auto-fixable on the [lint rule
reference](https://papyrus-lint.idrinth.de/rules.html), using the same
config's semicolon and
indentation settings) to each resolved script first, rewriting a script on
disk only if it changed, before reporting whatever diagnostics remain the
same way — the same repair the desktop app's "Fix" button applies to a
single script. A rewritten script is always saved back in the same
encoding it was read as (UTF-8 or Windows-1252), so fixing a file never
changes its encoding.

`fix` also accepts `--type <rule-id>` to apply only that one automatic fix
instead of every enabled one, and `--line <n>` to further restrict
whichever fix(es) run to just that 1-indexed line, leaving every other
line untouched — useful for an editor that wants to fix just the issue
under the cursor rather than the whole file. The rule id matches the one
in `[<rule>]`/`"rule"` in the plain-text or JSON report (e.g.
`trailing-whitespace`); `_` and `-` are interchangeable and matching is
case-insensitive, so `--type trailing_whitespace` also works. Both flags
are only valid alongside `fix` and can be combined. `--type` errors out on
a rule id that doesn't exist, or that exists but has no automatic fix
(e.g. `forbidden-functions`, which can only be reported); `--line` errors
out if applying the selected fix(es) would change the file's line count
(e.g. `property-sorting` relocating a property's declaration, or
`unused-import` removing a whole `Import` line), since a single original
line number no longer identifies the same line in the result in that case.

`fix` also accepts `--dry-run`, which computes the same fix(es) — honoring
`--type`/`--tag`/`--line` the same way — but never writes them to disk.
Instead, for each script that would change, a standard unified diff (the
same hunk format `diff -u`/`git diff` produce, three lines of context)
between the original and would-be-fixed source is printed as part of the
report, so you can review exactly what a real `fix` run would change
before actually running it. The diagnostics reported afterward still
reflect the would-be-fixed source, the same as a real `fix` run, so
`--dry-run` shows both what would change and what would still be left
once it did. With `--json`, each file's diff (when non-empty) is carried
in its own `diff` field instead of being interleaved with the plain-text
report, and the top-level report carries a `dry_run` boolean.

Every rule is also tagged with one or more kind keywords — `style`,
`performance`, `correctness`, or `maintainability` — describing what class
of fix its findings represent. `--tag <kind>` restricts a run to just one
of those kinds instead of a single rule id, matched case-insensitively
(e.g. `--tag style` or `--tag Performance`). Given without `fix`, it
limits the reported diagnostics to rules tagged with that kind; given
alongside `fix`, it also limits which automatic fixes run to that same
kind. Unlike `--type`/`--line`, `--tag` doesn't require `fix` — it works
just as well on a plain lint run. It can't be combined with `--type`,
since the two select overlapping things (one specific rule vs. one whole
kind of rule), and it errors out on a tag that doesn't match any rule's
kind keyword.

## JSON output

Given the `--json` flag (combinable with `fix`, in either argument order),
the CLI prints a single JSON document to stdout instead of the plain-text
lines and summary, so editor plugins and other tooling can consume the
report without scraping text. `--format json` is its equivalent;
`--format plain` explicitly selects the default output. `--format ai` instead
produces the same AI export as the desktop app: JSON containing the tool header,
findings, each affected file's source, and metadata for every triggered rule.
Given alongside `--format ai`, `--hash-source` replaces each file's exported
source with an md5 hash of its content instead of the full text — e.g. to
hand a report to an external AI assistant without exposing proprietary
script text, while a viewer can still tell files apart, or notice a file
changed between exports, from the hash alone. It's a usage error without
`--format ai`.
The normal JSON output contract is published as a
[JSON Schema](../schema/papyrus-lint-report.schema.json) using JSON Schema Draft 2020-12,
so integrations can generate types and validate saved or streamed reports:

```console
PapyrusLinterCLI --json --output report.json path/to/project.achlist
npx ajv-cli validate --spec=draft2020 -s schema/papyrus-lint-report.schema.json -d report.json
```

An example valid report is:

```json
{
  "files": [
    {
      "path": "scripts/source/Example.psc",
      "diagnostics": [
        { "line": 3, "column": 1, "rule": "trailing-whitespace", "level": "warning", "message": "[warning] Line contains trailing whitespace", "doc_url": "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" }
      ],
      "diff": null
    }
  ],
  "scripts_checked": 1,
  "files_with_diagnostics": 1,
  "total_diagnostics": 1,
  "files_fixed": null,
  "dry_run": false,
  "success": true
}
```

Every resolved script gets a `files` entry, even one with no diagnostics,
so a consumer can clear stale diagnostics for a file that's since become
clean. `level` is always `"error"`, `"warning"`, or `"info"`. Every built-in lint
sets a level; an untagged external diagnostic is conservatively reported as
`"error"` — see `Diagnostic::level`. Each diagnostic's `doc_url` is that
rule's own documentation link on the
[lint rule reference](https://papyrus-lint.idrinth.de/rules.html), or `null`
for a
rule with no known tag metadata (e.g. a compiler-reported diagnostic). `files_fixed` is
only present (non-`null`) when run with the `fix` subcommand, and under
`fix --dry-run` counts scripts that *would* have been fixed rather than
scripts actually rewritten on disk. `dry_run` reports whether this was a
`fix --dry-run` run; each file's `diff` is only non-`null` in that case,
for a script that would actually have changed, and carries the standard
unified diff between its original and would-be-fixed source. `success`
reports whether the run would exit `0`; here it's `true` because a
`[warning]`-level diagnostic doesn't fail the run under the default
`fail_on_warning: false`.

## Exit codes and version

It exits `0` if no diagnostics were found (or none of the ones found
counted as a failure — see `fail_on_warning`/`fail_on_info` in the
configuration reference), `1` if any did, or `2` on a usage error (a
missing/extra argument) or an I/O error (the `.achlist` or `.psc` file, or
one being fixed, couldn't be read/written/parsed) — so it can gate a CI
step on a clean lint run.

Run it with `--version`/`-V` to print its version (`PapyrusLinterCLI
<version>`) and exit `0` instead of linting; the desktop app shows its own
version next to its title.

Launched with no arguments, the desktop app's own executable starts its
GUI as normal; launched with any arguments, including an `.achlist` or `.psc`
path (or `-h`/`--help`), it routes all of them through the same CLI
implementation described above. Windows release builds use the console
subsystem so shells wait for CLI-mode completion and can reliably capture
plain-text or JSON stdout and stderr; when launched without arguments, the
executable detaches that console before starting the GUI.

## Getting the binary

Prebuilt `PapyrusLinterCLI`/`PapyrusLinterCLI.exe` standalone CLI binaries
for Linux, macOS, and Windows are attached to each [GitHub
release](https://github.com/Idrinth/papyrus-lint/releases), alongside the
desktop app's own bundles. To build the standalone CLI yourself instead,
run `cargo build --release --manifest-path
app/crates/papyrus-lint-cli/Cargo.toml`; the resulting binary is named
`PapyrusLinterCLI`.

The preferred way to run Papyrus Lint in CI is the [Papyrus Lint GitHub
Action](https://github.com/marketplace/actions/papyrus-lint)
(`idrinth/papyrus-lint-action`), which downloads `PapyrusLinterCLI` for you
and, on pull requests, posts findings as inline review comments on the
changed lines. See [`docs/github-actions-example.md`](github-actions-example.md)
for a usage example, and for a manual workflow that downloads a release's
`PapyrusLinterCLI` binary directly instead.

Each release also attaches a packaged copy of the two editor plugins: the
VS Code extension as a `.vsix` file (install via "Install from VSIX..." in
VS Code, or `code --install-extension <file>`) and the SublimeLinter
plugin as a `.zip` of the `SublimeLinter-contrib-papyrus-lint` directory
(extract it into Sublime Text's `Packages/` directory). Both still depend
on the standalone `PapyrusLinterCLI` binary above being installed and on
`PATH` (or configured via each plugin's settings).

See also the [Docker reference](docker.md) for running Papyrus Lint as a
container in CI without installing the binary at all.

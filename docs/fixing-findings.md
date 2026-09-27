# Fixing lint findings in the desktop app

Each `.psc` file listed on the Lint results tab that has at least one
finding for an auto-fixable lint (marked as such on the [lint rule
reference](https://papyrus-lint.idrinth.de/rules.html)) shows an "Apply
fixes" button, applying every automatic fix to that file at once — the
same repair the CLI's `fix` subcommand applies to a single script (see the
[command-line interface reference](cli.md)). Each individual finding for an
auto-fixable rule additionally shows its own "Fix this issue" button,
applying just that one finding's fix and restricting it to that finding's
own line, leaving every other line and finding untouched — the desktop
app's equivalent of the CLI's `fix --type <rule-id> --line <n>`. If that
fix would change the file's line count elsewhere (e.g. `property-sorting`
relocating a property's declaration, or `unused-import` removing a whole
`Import` line), it fails instead of applying anything, showing the error
inline next to the finding; the whole-file "Apply fixes" button has no
such restriction and always applies cleanly.

The code viewer has the same whole-file "Apply fixes" button built in,
next to "Edit" in its header, whenever the file it's currently showing has
at least one auto-fixable finding — applying the exact same repair and
refreshing the viewer's highlighted source in place, so fixing a file no
longer requires closing the viewer and going back to the Lint results
list first. It disappears again once nothing is left to fix.

Next to it, a "Preview fixes" button computes that same whole-file repair
without writing it to disk, rendering a standard unified diff (the same
hunk format `diff -u`/`git diff` produce) beneath the viewer instead — the
desktop app's equivalent of the CLI's `fix --dry-run`, for reviewing what
"Apply fixes" would change before actually running it. It shares "Apply
fixes"'s visibility (view mode only, and only while a fixable finding
remains) but never touches the file, the viewer's findings, or the Lint
results list; the shown preview is cleared again once you switch to Edit
or actually apply a fix.

While editing a script in the code viewer, its highlighted severities
update live as you type: a short pause after each edit re-lints the
editor's current (unsaved) text directly, in-process, the same lint pass
the CLI itself runs — so a fixed or newly introduced issue shows up
immediately instead of only after "Save". Until that first live lint
completes (or if it's still catching up with the latest keystroke), the
findings from the last save are shown instead, so the editor never goes
blank while you type. This is purely visual: nothing is written to disk,
and the project's Lint results list is only refreshed once you actually
save. Both the read-only viewer and the editor honor the Lint results
tab's active filters (severity, tag, rule, auto-fixable), so only the
findings still visible in the list are highlighted, titled, and offered
as per-line Fix/Ignore/File disable/Config disable actions. Changing a
filter while the viewer is open updates it in place.

Every line in the code viewer that has at least one finding also gets its
own small "Fix"/"Ignore"/"File disable"/"Config disable" buttons next to
it, in view mode. "Fix" only
appears when at least one of that line's findings is auto-fixable, and
applies each such finding's own fix restricted to that line, the same way
"Fix this issue" does — a rule whose fix would shift other lines (e.g.
`property-sorting`, or `unused-import` removing its own `Import` line) is
silently skipped rather than blocking the rest.
"Ignore" appears whenever at least one finding on the line carries a rule
id, and adds a [`; @disable <rule-id>[, <rule-id>...]`](annotations.md)
comment naming every rule found on that line instead of fixing it — merging
into an already-present `; @disable` comment on that line rather than
adding a second one, so clicking it again after a later run flags something
new just extends the same comment.
"File disable" appears next to "Ignore" and adds a
[`; @disable-file <rule-id>[, <rule-id>...]`](annotations.md)
comment naming those same rules instead, silencing them across the whole
file no matter which line the comment sits on — merging into an
already-present `; @disable-file` comment on that line the same way
"Ignore" merges into `; @disable`.
"Config disable" appears whenever at least one of the line's rules has a
`rules.*` switch in `papyrus-lint.yaml`, and turns those rules off in the
project's lint configuration (the same as unchecking them on the Settings
tab) rather than writing a disable comment — the currently open file is
re-linted immediately, and the rest of the Lint results list is marked
stale so switching back to it re-lints against the new settings.

A function header that returns a value or is `Native`, and isn't already
flagged, also gets its own "Nodiscard" button, regardless of whether that
line has any finding at all. Clicking it adds (or extends) a trailing
`; @nodiscard` comment on that header, marking the function so the
`unused-nodiscard` rule flags a caller that discards its result. The VS
Code extension offers the same action as a lightbulb Quick Action on an
eligible header, applying the edit directly to the buffer.

## Reviewing and exporting findings

The Lint results tab also has a "Mass fix an issue" panel, listing every
auto-fixable rule with at least one finding anywhere among the currently
loaded scripts (e.g. "Trailing whitespace (37)") next to a "Fix all ... in
project" button, applying just that one rule's fix across every one of
those files at once — the desktop app's equivalent of the CLI's
`fix --type <rule-id>` run against the whole project. A rule drops out of
the panel once none of its findings remain.

The Lint results tab also has an "Export issues" button, next to an
"Export format" selector (Text or JSON), that downloads every finding
currently passing the tab's own filters (filename search, severity, tag,
importance, auto-fixable, and rule) — exactly what's shown in the list
above it. The text format is one `<path>:<line>:<column>: [<rule>]
<message>` line per finding, the same layout the CLI's plain-text report
uses; the JSON format mirrors the shape of the CLI's own `--json` report
(a `files` array of `{path, diagnostics}`, plus `files_with_diagnostics`
and `total_diagnostics` counts), restricted to the currently filtered
files/findings, so both can be consumed by the same tooling. Each
diagnostic also carries a `doc_url` field — that rule's own documentation
link on the [lint rule reference](https://papyrus-lint.idrinth.de/rules.html),
or `null` for a
rule with no known tag metadata (e.g. a compiler-reported diagnostic) —
so a finding can be linked straight to its explanation. The button
is disabled whenever no finding currently passes the active filters.

Next to it, an "Export for AI" button downloads the same currently
filtered findings as a single JSON document tailored for handing to an AI
assistant alongside a question about the results, independent of the
"Export format" selector above (this format is always JSON, with its
contract published as a versioned JSON Schema:
[v5](../schema/papyrus-lint-ai-export.v5.schema.json), the current format
described below, and [v4](../schema/papyrus-lint-ai-export.v4.schema.json),
[v3](../schema/papyrus-lint-ai-export.v3.schema.json),
[v2](../schema/papyrus-lint-ai-export.v2.schema.json) and
[v1](../schema/papyrus-lint-ai-export.v1.schema.json), the frozen contracts
older releases produced, kept around so a document from an older release
can still be validated against the schema it was actually produced
under). The document's top-level `$schema` field points directly to that schema so an
assistant or validator can discover the exact contract without prior context. It contains
a `header`
identifying the tool name, running version,
[project website](https://papyrus-lint.idrinth.de) for further lookups, and
target game (`Skyrim SE/AE`), plus the UTC date and time at which the export
was generated; a `configuration` object containing the fully resolved lint
settings used for the run (including all defaulted values) - its
`enabled_rules` field lists just the hyphenated ids of the rules currently
switched on, alphabetically sorted, rather than repeating every rule's own
boolean flag (a rule not listed there is disabled); a
`filters` object recording the GUI's active filename, severity, importance,
rule, and auto-fixable-only filters, so the assistant can tell which findings
the user intentionally excluded — its `severities`, `importances`, and
`rules` arrays are never empty, since the Lint results tab refuses to let
every checkbox/option in one of those groups be deselected at once (an
empty array would otherwise be ambiguous between "the user excluded
everything" and "no restriction"), which also means the "Export
issues"/"Export for AI" buttons can never produce an export with zero
findings just by narrowing filters down to nothing; a
`findings` section in the same shape the "Export issues" JSON format uses
(minus its `files_with_diagnostics` count, always redundant here since every
exported file already has at least one diagnostic or parser error),
plus a `parser_errors` array on each file collecting any lexer or parser
errors raised while handling that script (empty when it lexed and parsed
cleanly; currently at most one entry, because parsing stops at the first
error), so an assistant can see why a file failed to parse instead of only
the lint findings that still ran;
with a `summary` of error, warning, and info counts both for every individual
file and for the complete export, while each file entry also carries a
`source` field explicitly naming
which of four forms it takes: `null` when no source was attached at all; an
object with `"type": "content"` carrying that script's full current on-disk
contents, so the assistant can see the exact code each diagnostic refers to
without needing the project's own files open alongside the report; an
object with `"type": "hash"` carrying an `algorithm` (currently always
`md5`) and `hash` instead, selected via the "Redact source (attach hash
only)" checkbox next to the "Export for AI" button (or the CLI's
`--hash-source` flag; see the command-line interface reference), so a report can be
handed to an external AI without exposing proprietary script text while the
assistant can still tell files apart, or notice a file changed between
exports, from the hash alone; or an object with `"type": "error"` carrying
a `message` describing why the source couldn't be read (e.g. it was moved
or deleted since linting) — each diagnostic raised by PapyrusCompiler.exe
itself (rule `compiler-error`, see [Compiling a script](compiling-scripts.md))
rather than
one of Papyrus Lint's own rules also carries `"external": true` and
`"source": "compiler"`, so the assistant can tell a compiler-reported
syntax error apart from an ordinary lint finding; and each diagnostic from an
auto-fixable rule also carries a `repair` field showing what that line
would look like after applying the rule's automatic fix, computed without
actually applying it — omitted when the fix wouldn't change that line at
all (e.g. `type-casing`'s own "no automatic fix" case) or would shift the
file's line count elsewhere (e.g. `property-sorting` relocating a
property's declaration), so an assistant can see a fix's effect without
asking the user to apply it first; a `doc_url` field on each diagnostic,
the same rule-documentation link "Export issues" carries above; and a
`rule_details` array carrying the rule metadata (kind(s), importance,
whether it is auto-fixable, its own `doc_url`, and the rule's own detailed
`description`, copied verbatim from its row in the
[lint rule reference](https://papyrus-lint.idrinth.de/rules.html)) for
every rule id
that actually appears among the exported findings and has known tag
metadata (an unrecognized rule id is simply left out) — giving the
assistant enough context about each triggered rule, in the same detail
the project documentation gives a human reader, to answer follow-up
questions precisely without needing this project's own documentation on
hand. Like "Export
issues", it's disabled whenever no finding currently passes the active
filters.

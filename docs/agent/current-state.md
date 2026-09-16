<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Current state


The parser (`app/crates/papyrus-parser`) understands scripts, imports,
properties (including full get/set property blocks), variables, functions
(including native/global/event functions and states), and expressions with
standard precedence. Each parsed `FunctionDecl` records the name of the
`State` block it was declared in (`None` for a function/event declared
directly on the script, i.e. the "empty state"), so downstream tooling can
tell a state override apart from its base declaration; `papyrus-lint-core`'s
`function_table.rs` carries that same `state` field through into its
cross-script function signatures, and includes a function declared only
inside a state (with no matching empty-state declaration) rather than
silently dropping it. Each parsed integer literal (`Literal::Int`) also
records the `IntFormat` (`Decimal` or `Hexadecimal`) it was written with,
so downstream tooling can tell a hex-written literal apart from a decimal
one without re-scanning the original source text; `papyrus-lints`'
`formid-hex-notation` lint reads the same distinction off the lexer's
`TokenKind::IntLiteral` tokens directly, since it works on tokens rather
than the parsed AST. The parsed `Script` also records the line its
`ScriptName` keyword starts on, and a parsed `Stmt::If` records the
line/column its `Else` keyword starts on (`None` when it has no `Else`
clause at all, distinguishing that from an empty `Else` clause, which both
leave `else_body` empty) — so `papyrus-lints`' `property-sorting` and
`empty-body` lints can read that information straight off the AST instead
of re-lexing the source, on a script that parses cleanly at all;
`empty-body`'s `Else` check still falls back to scanning tokens directly
when the script doesn't parse, since that's the only way it can still run
then.

`app/crates/papyrus-lints` currently implements all rules listed in the
[README's Implemented Lints table](README.md#implemented-lints). Rules inspect
raw source or lexer tokens rather than requiring a successfully parsed AST.
Automatic repair is available for trailing whitespace, comma spacing,
semicolons, indentation, whitespace around member-access dots, spacing
around `!` negation, spacing around logical/comparison operators, spacing
around assignment operators, property sorting (disabled by default;
see the README), and the unused-import lint. The desktop app,
standalone CLI, and editor extensions all use the same lint and repair
engine.

Unlike every other fixable rule, `unused-import`'s fix (`unused_import::repair_with`)
can only ever resolve which `Import` lines are actually unused through the
same project-wide `ExternalSignatures` resolver its own `check_with` already
needs (see the parser/lint architecture above): `papyrus_lints::repair`/
`repair_filtered`/`repair_filtered_by_tag` — which never see such a resolver
— leave it a no-op, the same way plain `unused_import::check` never flags
anything either. A caller that does have one calls the sibling
`repair_with_external_arguments`/`repair_filtered_with_external_arguments`/
`repair_filtered_by_tag_with_external_arguments` functions instead, mirroring
`lint_with_external_arguments`'s own split from plain `lint`. The CLI's
`fix` (via its per-script `SharedFunctionTable`) and the desktop app's
`repair_psc_file`/`repair_psc_finding`/`repair_psc_file_rule` Tauri commands
(via their own per-call `FunctionTable`, built before the fix instead of
after it) both use these external-aware entry points so "Apply fixes"/
`fix`/the mass-fix button actually remove a resolved-unused `Import` line;
`preview_repair_psc_file`/`preview_repair_psc_line` still call the plain,
resolver-less functions, so their previews never include this fix. Removing
a whole `Import` line always changes the file's total line count, so the
per-line "Fix this issue" button/`fix --line <n>` always rejects it as a
line-count-shifting fix, the same way `property-sorting` relocating a
property already does.

`papyrus-lints`' `tags` module publishes a `RuleTags` entry — kind
keyword(s) (e.g. `"style"`, `"performance"`, `"correctness"`,
`"maintainability"`), an `Importance` (`Low`/`Medium`/`High`) rating how
much fixing that rule matters for keeping a codebase maintainable, a
`description` copied verbatim from that rule's row in the README's
[Implemented Lints](README.md#implemented-lints) tables (kept in sync by
hand the same way `docs/nexuspage.bbcode`'s own lint descriptions are —
see "Keeping agent instructions synchronized" below), and an
`auto_fixable()` method derived from `FIXABLE_RULE_IDS` rather than stored
separately, so the two can never drift apart — for every id in
`KNOWN_RULE_IDS`, looked up case-insensitively via `tags::tags_for`. The
desktop app's `list_rule_tags` Tauri command (`app/src-tauri/src/lib.rs`)
exposes the same metadata to the frontend as a JSON-friendly
`RuleTagsInfo` per rule (its `description` is also what the Lint results
tab's "Export for AI" button carries in each `rule_details` entry — see
the README's Export for AI documentation); `app/src/main.ts` fetches it
once at startup
(`loadRuleTags`/`applyRuleTags`), indexes it by rule id, and uses it both
to render each lint finding's kind/importance/auto-fixable badges (see
`buildFindingTagsEl`) and to drive the Lint results tab's filters
(`matchesTagFilters`), alongside its existing severity and filename
filters. "Filter by tag / rule" combines what used to be a separate
flat "Filter by rule" multiselect with the tag kind checkboxes into one
fieldset (`populateRuleFilterGroups`): each kind (Style, Performance,
Correctness, Maintainability) gets its own multiselect listing just the
rules tagged with that kind — a rule tagged with more than one kind
(e.g. `argument-types`, tagged both `performance` and `correctness`)
appears in each of its kinds' own lists, kept in sync with each other
through the single `activeRules` set they all read from/write to
(`syncRuleFilterSelections`) — and the kind's own checkbox is a "select
all"/"select none" toggle for its multiselect (reflecting a partial
selection as indeterminate; `updateTagKindHeaderCheckbox`) rather than an
independent filter dimension of its own. The separate "Show
importance"/"Auto-fixable only" filters are unaffected. A finding whose
rule carries no tag metadata (e.g. a compiler-reported diagnostic; see
`app/crates/papyrus-lint-core/src/compile_diagnostics.rs`) always passes those filters
rather than being hidden. None of the "Show severities", "Filter by tag /
rule" (across every kind's multiselect combined, not per kind), or "Show
importance" groups can ever be left with every one of their own
checkboxes/options deselected — the last remaining one in a group refuses
to uncheck, reverting the DOM back to its prior state via
`syncRuleFilterSelections` for the rule case — since an empty `severities`/
`importances`/`rules` selection would otherwise be ambiguous between "the
user excluded everything" and "no restriction" once exported (see the
Export for AI `filters` object below), and would silently make "Export
issues"/"Export for AI" produce an empty export. The CLI's `--tag <kind>`
flag builds on the same metadata to run only one kind's worth of
lints/fixes at a time,
matched case-insensitively against a rule's `kinds` (e.g. `--tag style`):
given without `fix`, it restricts the reported diagnostics to matching
rules; given with `fix`, it also restricts which automatic fixes run, via
`papyrus_lints::repair_filtered_by_tag` (a sibling of `repair_filtered`,
which does the same for a single rule id via `fix --type`, both built
atop a shared private `repair_with` that takes an `applies(rule) -> bool`
predicate). `--tag` can't be combined with `--type`, since the two select
overlapping things (one rule vs. one kind of rule), and an unrecognized
tag is a usage error.

Each `RuleTags` entry also carries a `doc_slug` — the anchor id
(`pages/build.py`'s `lint-<slugify(name)>`, built from that rule's own row
in the README's Implemented Lints tables) its row renders under on the
project website's homepage — kept in sync by hand the same way
`description` is, since it's derived from the README row's display name
rather than the rule's id and the two don't always match (e.g.
`comma-spacing`'s row is titled "Space after comma", so its slug is
`space-after-comma`). `RuleTags::doc_url()` builds the full
`<website>/#lint-<doc_slug>` link from it, giving every surface that
already carries rule tag metadata a way to jump a user straight to that
rule's own documentation instead of just naming it: the desktop app's
`list_rule_tags` command includes `doc_url` in each `RuleTagsInfo`, and
`buildFindingTagsEl` renders it as a "docs" badge/link alongside a
finding's kind/importance/auto-fixable badges; the CLI's plain-text report
appends it after a diagnostic's message (`format_diagnostic_line`) when
the rule has known tag metadata; and both the CLI's `--json`
(`JsonDiagnostic::doc_url`, `null` for a rule with none) and `--format ai`
(`AiRuleDetails::doc_url`, always present since untagged rules are
filtered out of `rule_details`) output, and the desktop app's matching
"Export issues"/"Export for AI" JSON, carry the same field — the AI
export's `$schema` was bumped to
[v3](docs/papyrus-lint-ai-export.v3.schema.json) for it, with
[v2](docs/papyrus-lint-ai-export.v2.schema.json) (and
[v1](docs/papyrus-lint-ai-export.v1.schema.json)) kept frozen alongside it
the same way v1 was kept when v2 introduced the `repair` field. The VS
Code extension turns a diagnostic's `doc_url` into a clickable
`{value, target}` diagnostic code (`toDiagnostic`/`ruleOfDiagnosticCode` in
`vscode-extension/src/extension.ts`) instead of a plain string rule id
when one is known, and the SublimeLinter plugin appends it to the
diagnostic's message text (`_to_lint_match` in
`SublimeLinter-contrib-papyrus-lint/linter.py`), since neither
SublimeLinter's own diagnostic model nor Sublime Text's popups have a
first-class documentation-link mechanism the way VS Code's diagnostic code
does.

The CLI also accepts `--blob <source>` in place of an achlist/`.psc`/
directory path: `run_blob` in `papyrus-lint-cli/src/lib.rs` lints `<source>`
directly, via plain `papyrus_lints::lint` (no
`ExternalSignatures`/`FunctionTable`, since there's no project root to
resolve other scripts against), and reports it under the literal path
`<blob>`. This is for a script buffer that isn't (yet, or ever) saved to
disk, e.g. from an editor extension that already has the text in memory,
so it skips every piece of project-level machinery a normal run needs
(project root discovery, cross-script argument/return type resolution, the
`conflicting_script_versions`/`stale_compiled_output`/
`script_filename_mismatch` project lints, `compile_check`). `--config
<path>` still selects an explicit configuration file to lint the blob
against; omitted, `papyrus_lints::Config::default()` applies, since there's
no project root to discover one from. It shares `--tag`'s own normalization/
validation (factored into `normalize_tag_filter`, used by both the normal
run and `run_blob`) and reuses the same `JsonReport`/`AiReport` shapes a
normal run produces, just with a single `<blob>`-named file entry. `--blob`
is a usage error combined with a path argument, `fix`, `--type`, `--line`,
`--dry-run`, `--script-root`, `--progress`, or `--threads` — none of which
mean anything without a real file to resolve scripts around or write fixes
back to.

Both editor plugins use `--blob` for live, as-you-type feedback on a
document's current (possibly unsaved) contents, alongside their existing
open/save linting of the real file. The VS Code extension's
`PapyrusLinter.lintBlob` (`vscode-extension/src/extension.ts`) runs
`PapyrusLinterCLI --json --blob <text>` against `document.getText()`,
triggered from a new `onDidChangeTextDocument` listener and debounced per
document (`scheduleLiveLint`/`liveLintTimers`, cleared on document close)
by the `papyrusLint.liveLintDebounceMs` setting (default `400`ms); the
`papyrusLint.liveLint` setting (default `true`) turns this off entirely. A
live lint's own failure (CLI launch, usage error, malformed JSON) is only
logged to the "Papyrus Lint" output channel, never shown as an error
message box, unlike every other CLI invocation this extension makes — it
runs unattended on every pause in typing, so popping a message box on a
transient failure (e.g. a download hiccup) would be disruptive;
`PapyrusLinter.applyResult`'s new `notify` parameter (default `true`)
controls this. The SublimeLinter plugin instead switches per-invocation,
in `PapyrusLint.cmd` (`SublimeLinter-contrib-papyrus-lint/linter.py`):
`self.view.is_dirty()` selects between the existing `${file}` argument (a
saved view, unchanged) and `--blob <buffer text>` (read via
`self.view.substr(sublime.Region(0, self.view.size()))`) for an unsaved
one, needing no debounce logic of its own since SublimeLinter's own
background linting (per the user's standard `lint_mode` setting) already
reruns `cmd()` as the buffer changes. In both cases, and the same way
`run_blob` itself documents, a live/unsaved lint only honors an explicit
`config_path`/`papyrusLint.configPath` override — it can't discover the
project's own `papyrus-lint.yaml`/`.yml` or run cross-script checks, since
`--blob` skips project-root discovery entirely; the full, project-aware
lint still runs again once the file is actually saved.

The desktop app's code viewer gets the equivalent of this for its own Edit
mode: `app/src/main.ts`'s `scheduleLiveEditLint`, wired to the edit
textarea's `input` listener alongside the existing highlight/autocomplete
handlers, debounces (`LIVE_EDIT_LINT_DEBOUNCE_MS`, `400`ms) a call to
`runLiveEditLint`, which lints the textarea's current value via the new
`lintPapyrusScript` wrapper around the `lint_papyrus_script` Tauri command
(`app/src-tauri/src/lib.rs`, already existing but previously unused by the
frontend) — the same in-process `papyrus_lints::lint` call the CLI itself
ultimately runs, taking the place of shelling out to a CLI subprocess the
way the two editor plugins above do. Unlike the CLI's `--blob`, this isn't
combined with an explicit config override at all; it always lints against
`currentLintConfig`, the same configuration every other lint/fix Tauri
call in the app already uses. Its result populates
`codeViewerEditLiveFindings`, a new module-level findings list
`updateCodeViewerEditHighlight` reads from while in edit mode (seeded from
`codeViewerState.findings` — the last on-disk lint — by
`enterCodeViewerEditMode`, so the highlighted severities never go blank
while a first live lint is still in flight), in place of the
always-stale-until-save `codeViewerState.findings` it read before. A
`codeViewerLiveLintRequestId` guard (the same stale-response pattern
`autocompleteRequestId` already uses for `updateAutocomplete`) discards a
response that resolves after a newer edit started a fresher one, or after
edit mode was already left; `cancelLiveEditLint` — called from
`setCodeViewerMode` whenever leaving edit mode, and exported for tests to
reset this timer between runs — cancels a still-pending debounce outright
and bumps that guard so an in-flight request can no longer apply either.
Like the CLI's `--blob`, this is a purely local, in-editor lint: it never
writes to disk, and the project's Lint results list is only refreshed once
the edit is actually saved (`persistCodeViewerEdits`, unchanged).

Edit-mode hover and `.`-triggered autocompletion also surface the `{ ... }`
documentation comments that `missing-doc-comment` already checks for.
`papyrus_lints::missing_doc_comment::documentation_comment` extracts the
inner text (same last-physical-line / backslash-continued-header placement
as the lint itself); `FunctionTable` carries it on each
`FunctionSignature`/`PropertySignature` `doc` field so `list_script_members`
returns it for members resolved from disk (including inherited ones). The
frontend (`app/src/autocomplete.ts`) additionally scans the unsaved buffer
so a comment typed since the last save still shows: hovering a ScriptName /
Property / Function / Event header (or a use of that name) puts the comment
in the textarea's `title` above any lint findings for the line, and the
active autocompletion item renders it under its signature.

Project configuration is read from an optional `papyrus-lint.yaml` or
`papyrus-lint.yml` in the project root. Both the desktop app and the CLI are
forgiving of an achlist that doesn't live in the project root itself (e.g.
dropped next to a game's `Data` directory while the project lives in a
subfolder): each first tries to find the root from where the achlist's own
resolved scripts sit under a `Scripts/Source` or `Source/Scripts` pair
(`projectDirForAchlist` in `app/src/main.ts`; `find_candidate_pair_root` in
`papyrus-lint-cli`), falling back to the achlist's own parent directory (the
conventional layout) only if none of them do. The CLI resolves a bare `.psc`
given directly the same way, walking up from the file itself and falling
back to two directories above it if no such pair is found at all; the
desktop app's frontend still uses that simpler fixed "two directories up"
rule for a bare `.psc` dropped directly (`projectDirForPscPath`).

Given a directory instead of an `.achlist`/`.psc` path, the CLI
recursively scans it (and every subdirectory beneath it, at any depth)
for `.psc` files instead
(`papyrus_lint_core::script_locator::find_psc_files_recursively`) and
lints every one found — for a project (e.g. Requiem's own layout) with no
`.achlist` at all whose scripts are spread across arbitrarily nested
subfolders rather than a flat `scripts/source`. Its project root is found
the same way as an achlist's: each resolved script is tried against
`find_candidate_pair_root` first, falling back to the scanned directory
itself (rather than an achlist's parent directory, which doesn't apply
here) only if none of them match. The desktop app exposes the same scan
as a `list_psc_files_recursively` Tauri command, used by
`handleDroppedPaths` in `app/src/main.ts` when a single dropped path is
neither an `.achlist` nor a `.psc` file (it errors out for a path that
isn't an existing directory either, which the frontend treats the same as
today's "drop a single .achlist or .psc file" case); `projectDirForDirectory`
mirrors `find_candidate_pair_root`'s fallback using the same
`findCandidatePairRoot` helper `projectDirForAchlist` already uses.
Cross-script resolution across the discovered subfolders works the same
way it does for achlist entries, since both share the code path that adds
each resolved script's parent directory as an additional search root.

By default, the CLI (and the desktop app) resolves cross-script lookups
among an achlist's own entries by treating every listed entry's parent
directory as a generic additional search root — which matters for achlists
whose entries are grouped into arbitrary source directories rather than
either conventional layout, since a script in one listed directory then
still resolves a script type declared in another listed directory. This
also means an unlisted file that happens to sit in one of those
directories resolves too, and `conflicting_script_versions` scans every
such directory in full, which got expensive on a modlist-sized achlist
(hundreds of listed files across as many directories; see
[#311](https://github.com/Idrinth/papyrus-lint/issues/311)). The CLI avoids
repeating that scan once per listed script by building a
`script_locator::ScriptIndex` (`build_script_index`) — a one-time map from
each directory's file names to their paths — up front, then using it both
for cross-script name resolution and to check every script via
`conflicting_script_versions_in_index` instead of
calling `conflicting_script_versions` (which scans afresh on every call)
per script; the desktop app's single-file `lint_psc_file`/`repair_psc_file`
commands still call `conflicting_script_versions` directly, since they
only ever check one script per invocation. Setting the project's
`strict_achlist_scope` to `true` switches the CLI to registering
each listed `.psc` directly with the `FunctionTable`
(`FunctionTable::with_known_scripts` in `papyrus-lint-core`) instead,
scoping resolution strictly to what the achlist actually lists — cross-listed-directory
resolution still works, but nothing unlisted leaks in, and no directory is
scanned at all — with `script_locator::conflicting_script_versions_among`
covering the one case directory scanning otherwise catches for free: two
listed entries sharing a file name. It defaults to `false` so an existing
achlist-based project's resolution/diagnostics don't change underneath it.

The CLI's per-script lint loop (and `fix`) reads, fixes, and lints multiple
scripts at once instead of one at a time, via
`papyrus_lint_core::parallel::map_in_parallel` — a small, dependency-free
worker pool (`std::thread::scope` plus a shared work queue) that hands
results back in the scripts' original order regardless of which order the
worker threads actually finish them in, so the plain-text/JSON/AI report
(and a `--progress` bar's own file-count sequence) is identical no matter
the thread count. The number of workers is controlled by `--threads <n>`
(a positive integer; defaults to `parallel::default_thread_count()`, the
machine's available parallelism), with `--threads 1` forcing the previous
fully sequential behavior. Every script is otherwise independent, so the
one thing worker threads actually share is the run's single
`FunctionTable` (cross-script argument/return type lookups): it's wrapped
in a `Mutex` and accessed through `function_table::SharedFunctionTable`, an
`ExternalSignatures` adapter that locks only for the duration of one
lookup (each forwarded through the `ExternalSignatures` trait itself via
fully qualified syntax, so it can't drift from `FunctionTable`'s own trait
impl) rather than for a whole script's lint pass — since `FunctionTable`
caches everything it resolves, that's typically one lock acquisition per
referenced type, not per lookup. This is also why `ast_cache`'s own
accessors (`get`/`put`/`get_tokens`/`put_tokens`/`ensure_primed`) serialize
on a single process-wide lock: `std::fs::write` isn't atomic, and two
scripts linted at once can both need the same cross-script dependency's
cache entry at the same moment; a corrupted read already fell back to a
fresh parse before this (caching is a pure optimization — see
`ast_cache`'s own module docs), so this closes a wasted-reparse gap rather
than a correctness one, and covers the desktop app's own per-file Tauri
commands too, which were already running concurrently across a batch drop
(see below) without this. The desktop app's own frontend caps how many
scripts it works on at once the same way, in `parsePscFiles`
(`app/src/main.ts`): `mapWithConcurrency` bounds concurrent `parse_psc_file`/
`lint_psc_file` invocations to `parseConcurrencyLimit()` (the browser's
`navigator.hardwareConcurrency`, or `4` if that's unavailable), rather than
firing every resolved script's pair of Tauri commands at once — each
dispatched to its own thread by Tauri, so far more in flight than the
machine has cores to run them on just adds contention without finishing
sooner. This mirrors the CLI's own `--threads` default without changing
`parsePscFiles`'s existing contract: results still resolve in `paths`' own
order and `onOutcome` still fires in completion order as each script
finishes.

Every Tauri command that touches the filesystem, parses/lints/repairs a
script, or spawns PapyrusCompiler.exe is declared `#[tauri::command(async)]`
in `app/src-tauri/src/lib.rs`, rather than a plain `#[tauri::command]`. A
synchronous command with no `async`/`(async)` marking is dispatched inline
on Tauri's main/UI event-loop thread, so without this attribute a single
lint/repair/compile call — and the concurrent batch described above —
blocks window rendering and input until it returns, which is what made the
app appear to hang while linting a large `.achlist`/directory drop.
`(async)` keeps each command's own Rust signature an ordinary synchronous
`fn` (so the `#[cfg(test)]` module further down still calls every one of
them directly, with no `.await`) while making Tauri route its actual
dispatch through `tauri::async_runtime::spawn_blocking`, onto its blocking
thread pool — which is what lets the batch concurrency described above
genuinely run each invocation on its own thread rather than queueing on the
UI thread. `get_app_version` and `list_rule_tags` are the only two commands
left as plain `#[tauri::command]`, since both just return an in-memory
constant/static table and complete instantly on the main thread regardless.

The desktop app picks each project's lint configuration explicitly, right
after a drop resolves which project directory is actually in play, rather
than showing/editing whatever configuration happened to be loaded
previously (or the engine's silent defaults) before the user has even said
which project it applies to. `loadProjectConfig` (`app/src/main.ts`) is
what `handleDroppedPaths` calls, in place of calling `useProjectDir`
directly: for a project directory not yet confirmed this session, it locks
the entire Settings tab (`setSettingsLocked`, backed by a `<fieldset
id="settings-fieldset" disabled>` wrapping every Settings tab control, plus
a `#settings-locked-notice` paragraph explaining why) and shows the
`#config-picker` dialog (`promptForConfigSelection`) before doing anything
else. "Continue" (or Escape, or a backdrop click) accepts whatever
`useProjectDir`'s own auto-detection would already do — the project's
existing `papyrus-lint.yaml`/`.yml`, or the engine's silent defaults if it
has none; an inline list of presets (shown only when the project has none
yet — see below) lets the user click one to start from instead; and a text
field lets the user point at a specific configuration file instead. The
Settings tab's own "Configuration
file" input (`configPathOverrideEl`) is set to that typed path (or cleared,
for the other two choices) before `useProjectDir` actually loads and
applies the resulting configuration and the Settings tab is unlocked. A
project directory already confirmed this session (e.g. dropping the same
achlist again) skips the dialog entirely and reuses whatever was picked the
first time. `useProjectDir` itself stays a plain, reusable "load this
already-decided directory's configuration" function with no dialog of its
own, since other call sites — the "Configuration file" input changing,
once the tab is already unlocked — need to reload a project's
configuration without re-asking which one to use. The app doesn't restore
whichever project directory was last open at startup: the Settings tab
simply stays locked (see `setSettingsLocked`) until the user actually
drops something this session. Silently reloading a remembered directory's
configuration on startup would have been pointless anyway — nothing marks
that directory confirmed, so dropping that same project again still goes
through the picker dialog, which then picks (and applies) its
configuration itself, overwriting whatever the silent restore had just
loaded.

Editing the "Configuration file" input directly, once unlocked, still
overrides project-directory discovery entirely the same way it always has:
when set, the app reads/writes lint settings at that exact path instead —
via the `load_lint_config_from_path`/`save_lint_config_to_path` Tauri
commands, which wrap `papyrus_lint_core::config::load_config_from_path`
(also used by the CLI's own `--config <path>`) and
`config::save_config_at_path`. Unlike before, it's no longer remembered in
`localStorage` independent of the loaded project: a value chosen for one
project has no bearing on the next one dropped, since each project's
configuration is picked fresh via the dialog above rather than a page-load
default the user could edit before dropping anything. Leaving it blank
reverts to the normal auto-detection described above. Editing lint
settings while an override is active saves to that file (creating it if it
doesn't exist yet, preserving any other settings — e.g. `compiler_path` —
already stored in it) instead of the current project directory's own
config file; `compiler_path`, `compile_check`, and
`additional_script_roots` themselves are unaffected by this override and
still follow the loaded project directory.

Configuration controls formatting, lint enablement, complexity thresholds,
CLI failure levels, and the compiler path. It also controls whether the
desktop app's `lint_psc_file`/`repair_psc_file` commands, and the CLI's own
per-script lint loop, additionally run PapyrusCompiler.exe against a `.psc`
as part of linting it (`compile_check`, off by default — the CLI reads it,
and the resolved `compiler_path`, from the project root's own config the
same way its `doctor` subcommand already did, regardless of `--config`),
merging in any errors it reports (see
`app/crates/papyrus-lint-core/src/compile_diagnostics.rs`) alongside the
lint engine's own; unlike the "Compile"/"Save & Compile" buttons, this
always compiles into a throwaway temporary directory rather than the
project's real output directory.

The same `lint_with_compile_check`/CLI per-script lint loop also runs the
"Stale compiled output" project lint (`rules.stale_compiled_output`, on by
default; see `app/crates/papyrus-lint-core/src/stale_pex.rs`) whenever
linting a `.psc` with project context: it compares the script's own
last-modified time against its conventionally located compiled `.pex`
(the source directory's own parent, e.g. `Scripts/Example.pex` for
`Scripts/Source/Example.psc` — the same location `compiler.rs` compiles
to), reporting an `[info]` diagnostic when the script is newer, since
that usually means someone edited it and forgot to recompile. A script
with no `.pex` there yet (never compiled, or compiled somewhere else)
isn't flagged — this only compares timestamps once both files are known
to exist.

See the [README configuration
reference](README.md#configuration) for the per-key documentation, and
[`docs/papyrus-lint.default.yaml`](docs/papyrus-lint.default.yaml) — the
same file `PapyrusLinterCLI init` writes and the one the README links to
instead of dumping inline — for the complete default file. That file must
stay byte-for-byte identical to `PapyrusLinterCLI init`'s output (built
from `papyrus_lints::Config::default()` and the `FIELD_COMMENTS` table in
`papyrus-lint-core/src/config.rs`, which is what actually generates the
per-key comments): `papyrus-lint-core`'s
`config::tests::default_config_matches_the_checked_in_docs_copy` test
fails CI if they drift, so regenerate it with `PapyrusLinterCLI init`
(and update `FIELD_COMMENTS`/README together) whenever a default or a
field comment changes.

`PapyrusLinterCLI init` also accepts `--preset <strict|standard|careful|name>`
(matched case-insensitively, defaulting to `strict`), which selects the
baseline `config::Preset` (`papyrus-lint-core/src/config.rs`) it generates
`papyrus-lint.yaml` from, in place of the hardcoded default. `strict` is
identical to `papyrus_lints::Config::default()` (and to
`docs/papyrus-lint.default.yaml`), so plain `init` — no `--preset` — is
unaffected by the flag existing at all; `standard` and `careful` are less
noisy. Each built-in preset's own annotated YAML lives under
[`docs/presets/`](docs/presets/) (`papyrus-lint.strict.yaml`,
`.standard.yaml`, `.careful.yaml`) and is compiled into the binary via
`include_str!`, rather than read from disk at runtime.

Any other name is resolved as a user preset instead:
`config::Preset::parse` accepts any non-blank name that isn't one of the
three built-ins as `Preset::Custom(name)` without touching the filesystem
yet, and `Preset::yaml(base_dir)` — called once `init` actually needs the
preset's YAML — looks for a `<name>.yaml`/`.yml` file (matched
case-insensitively via `config::find_user_preset_file`) inside a `presets`
directory (`config::USER_PRESETS_DIR_NAME`) next to `base_dir`
(`config::user_presets_dir`/`user_presets_dir_under`), erroring out if
`base_dir` is unavailable or no such file exists there. This mirrors the
executable-adjacent base config below: both live next to the same
executable, resolved through the same `base_dir`/`executable_dir()` split so
tests can supply a controlled directory instead of depending on the test
binary's own `current_exe()`. `config::list_user_preset_names(dir)` lists
every such file's stem (sorted case-insensitively), for the desktop app's
preset picker below.

The CLI's `preset add <name> <path-to-papyrus-lint.yaml> [--yes]` subcommand
creates a user preset from an existing config file, rather than requiring
one to be placed under the executable-adjacent `presets` directory by hand:
`config::add_user_preset(name, source_path, overwrite)` copies
`source_path`'s contents into that directory as `<name>.yaml` (creating the
directory first if needed), refusing `name` if it's blank or matches a
built-in preset name case-insensitively (`AddPresetError::InvalidName`),
since such a name could never actually be selected — `Preset::parse` always
resolves a built-in first. If a preset named `name` already exists there
(as either `.yaml` or `.yml`), it's left untouched and
`AddPresetError::AlreadyExists` is returned unless `overwrite` is `true`;
the CLI surfaces this as `--yes`, the confirmation an overwrite requires
since the CLI has no interactive prompt, and reuses the existing file's own
extension when overwriting rather than creating a second file alongside it.
Split into a private `add_user_preset_under(base_dir, ...)` the same way as
`initialize_default_config`/`initialize_config_with_base` above, so tests
can supply a controlled directory instead of depending on the test binary's
own `current_exe()`.

`papyrus-lint-cli`'s `doctor <path-to-achlist-or-psc-or-directory>`
subcommand (`run_doctor` in `papyrus-lint-cli/src/lib.rs`) validates a
project's setup — the paths its configuration assumes or names — without
linting any script. It accepts the same `--config`/`--script-root` flags a
plain lint/fix run does, so it reports on exactly the project a matching
run would actually use, and resolves the achlist/`.psc`/directory input
and the project root the same way `run` does
(`find_candidate_pair_root`/`find_psc_project_root`). Each check (the
input path itself existing; every listed `.achlist` entry existing on
disk; the discovered or `--config`-named config file parsing; at least one
of `scripts/source`/`source/scripts` existing under the project root;
each configured `additional_script_roots`/`--script-root` entry resolving
to an existing directory; a configured or auto-detected `compiler_path`
pointing at an existing file) is collected as a `DoctorCheck` — an
`ok`/`warning`/`error` `DoctorStatus` plus a message — rather than
aborting the run on the first problem found, so a single invocation
reports the full picture at once. `compiler_path` being unset and
unauto-detectable is only ever a `warning` when `compile_check` is also
enabled (checked separately, via `resolve_compiler_path`) — otherwise it's
harmless and reported as `ok`, since the CLI itself never needs
`compiler_path` outside that setting. Printed as one `[ok]`/`[warning]`/
`[error] <message>` line per check in the plain-text report, or as a
`DoctorReport` (`project_root`, `checks`, `success`) JSON document with
`--json`; exits `0` if every check passed, `1` if any reported a `warning`
or `error`, or `2` on a usage error, the same convention every other
subcommand follows.

`initialize_default_config` also looks for a `papyrus-lint.yaml`/`.yml`
file next to the running executable (`config::executable_dir`, backed by
`std::env::current_exe()`) and, if one exists, layers it over the selected
preset's own YAML instead of using the preset alone: any key it sets
overrides the preset, any key it omits still falls back to the preset.
This is a recursive per-key merge over `serde_yaml::Value` (`deep_merge`)
rather than serde's own `#[serde(default)]` handling, since which
"default" a key falls back to now depends on the chosen preset at runtime
instead of being fixed at compile time; the merge covers `rules:`'s own
nested keys too, so a base file that only overrides a couple of individual
rules still inherits every other rule from the selected preset. This lets
someone define their own baseline settings once, next to wherever they
keep the CLI or desktop app binary, and reuse it across every project they
run `init` in — layered on top of whichever preset they pick each time —
rather than hand-editing each newly generated file the same way
afterward. The lookup is split into a private `initialize_config_with_base(dir,
base_dir, preset)` so tests can supply a controlled `base_dir` instead of
depending on the test binary's own `current_exe()`; the checked-in
`docs/papyrus-lint.default.yaml` copy is unaffected since CI's test
environment has no such file next to the test binary, and the `strict`
preset (`init`'s own default) reproduces it byte-for-byte.

The desktop app offers the same presets from its own config-picker dialog
above, rather than only through the CLI's `init --preset` flag:
`promptForConfigSelection` fetches every preset via the
`list_config_presets` Tauri command (only when the project has no
`papyrus-lint.yaml`/`.yml` yet, per `load_project_info`'s result — a preset
is pointless to offer once one's already been detected) and renders one
button per preset directly into the dialog's own `#config-picker-preset-list`
(hidden entirely when the list comes back empty), rather than opening a
second, nested dialog for it: the preset list is only ever meaningful as
part of this one choice, so there's nothing else it needs to be its own
dialog for. `list_config_presets` is backed by `papyrus-lint-core`'s
`presets` module (`presets::all()`) — a thin label/description layer over
`config::Preset`/`config::PRESET_NAMES`, the same enum the CLI flag parses,
which appends a `PresetInfo` (id/label both the file's stem, a generic
description) for every name `config::list_user_preset_names` finds under
the executable-adjacent `presets` directory (`config::user_presets_dir`),
after the three built-ins; `presets::all()`/`PresetInfo`'s fields are owned
`String`s rather than `&'static str`, since a user preset's identity is
discovered from a file name at runtime instead of being a compile-time
constant. Clicking one resolves `promptForConfigSelection` with
`{ kind: "preset", preset: id }`, which `loadProjectConfig` then hands to
`apply_config_preset(dir, id)` — resolving the id via `config::Preset::parse`
and handing it to `config::initialize_default_config`, the very function
`init --preset` itself calls — so the desktop app gets the same "refuse to
replace an existing config" guard, executable-adjacent base-config
layering, and user-preset resolution for free.

The Settings tab's own "Save current settings as preset…" button goes the
other direction: `handleSaveConfigAsPresetClick` (`app/src/main.ts`) prompts
for a name, then — if `list_config_presets` already lists a preset under it
(matched case-insensitively; a built-in name is rejected by the backend
outright, since `config::Preset::parse` always resolves those first) —
confirms overwriting it before calling the `save_config_as_preset` Tauri
command with the currently edited `LintConfig`, the name, and whether to
overwrite. That command wraps `papyrus-lint-core`'s
`config::save_user_preset`, which writes just the lint settings (not a
project's own `compiler_path`/`additional_script_roots`/`compile_check`/
`strict_achlist_scope`, which aren't something a reusable preset should
hardcode) as `<name>.yaml` under the same executable-adjacent `presets`
directory the picker above and `list_user_preset_names` read from —
creating that directory first if it doesn't exist yet — so the saved
preset is immediately selectable from the picker, or via the CLI's
`--preset <name>`, without restarting anything.

A "Presets" tab (`#tab-presets`/`#panel-presets`, next to Settings) lets a
user rename, export, or delete their own saved presets afterward, without
touching the filesystem by hand. It's only shown once at least one exists:
`renderPresetManagementTab` (`app/src/main.ts`) filters whatever
`list_config_presets` returns down to the non-built-in ones
(`isCustomPreset`, checking a preset's id against the three built-in names
by hand — the same convention `FIXABLE_RULE_IDS` follows for the lint
engine's own fixable rule ids), hides the tab entirely when that list is
empty, and switches back to the Settings tab if it was the active one and
its last preset just disappeared. `refreshPresetManagementTab` re-fetches
and re-renders it, called once at startup and after every action below
that could change which presets exist (including
`handleSaveConfigAsPresetClick` itself, above). Each listed preset gets
three buttons:

- **Rename** (`handleRenamePresetClick`) prompts for a new name, applying
  the same "cancel on a blank prompt" and "confirm before overwriting an
  already-used name" rules `handleSaveConfigAsPresetClick` does, then
  calls the `rename_user_preset` Tauri command
  (`papyrus_lint_core::config::rename_user_preset`), which finds the
  preset's existing `<name>.yaml`/`.yml` file in the executable-adjacent
  `presets` directory and renames it in place — refusing a blank or
  built-in new name the same way `save_user_preset` does, and preserving
  the file's own extension.
- **Delete** (`handleDeletePresetClick`) confirms, then calls
  `delete_user_preset` (`config::delete_user_preset`), which removes the
  matching file from the `presets` directory.
- **Export** (`handleExportPresetClick`) calls `export_user_preset`
  (`config::read_user_preset_yaml`), which returns the preset's file
  contents verbatim (unlike `initialize_default_config`, it isn't merged
  against an executable-adjacent base config or a project's own settings),
  and downloads it as `<id>.yaml` via the same Blob-and-anchor technique
  `handleExportIssuesClick` uses for the Lint results tab's own export
  button, so it needs no Tauri fs/dialog plugin either. Built-in presets
  can't be renamed, deleted, or exported this way, since none of the three
  backend functions above ever resolve a name matching
  `config::PRESET_NAMES`.

The Settings tab also has a "Reset to preset…" control (a
`#reset-to-preset-select` dropdown, listing every preset the same way the
"Reset" button next to it is populated by `populateResetPresetSelect` —
kept in sync with the Presets tab via `refreshPresetManagementTab`, since
both list the same presets) for undoing a settings change gone wrong,
rather than hand-editing every field back: `handleResetToPresetClick`
confirms overwriting the currently edited settings (since, unlike
`apply_config_preset`, this can discard settings already saved to a
project's real config file, not just a scratch in-memory edit), then
fetches the selected preset's lint settings via the `get_preset_lint_config`
Tauri command and applies them through the exact same
`applyLintConfigToUI`/`handleLintConfigChanged` path any manual field edit
already goes through, so the reset is written wherever settings are
already being saved (the current project directory, or an active
"Configuration file" override) with no separate save codepath of its own.
`get_preset_lint_config` wraps `papyrus-lint-core`'s new
`config::preset_lint_config_default` — a sibling of
`initialize_default_config` sharing its preset-resolution and
executable-adjacent base-config-layering logic (`resolve_preset_project_file`)
but returning just the resolved `papyrus_lints::Config` instead of writing
a brand new project file, and neither refusing an already-existing config
nor touching a project's own `compiler_path`/`additional_script_roots`/
`compile_check`/`strict_achlist_scope` settings, since those aren't
something resetting a project's *lint* settings back to a preset should
touch.

The desktop app's `parse_psc_file` command, both the app's and the CLI's
cross-script lookups (`papyrus-lint-core`'s `function_table.rs`, used to
resolve the "Argument type check"/"Return type check" lints across
scripts), and the desktop app's `lint_psc_file`/`repair_psc_file`/
`repair_psc_finding`/`repair_psc_file_rule` commands and the CLI's own
per-script lint loop (via `ast_cache::ensure_primed`, see below) cache
each parsed `.psc` AST on disk
(`app/crates/papyrus-lint-core/src/ast_cache.rs`), in an `ast-cache`
directory next to the running executable — the desktop app's own binary,
or `PapyrusLinterCLI`'s, whichever process is doing the parsing. A cached
entry is only reused when its stored MD5 of the file's content and the
file's last-modified timestamp still match, and the linter version that
wrote the entry is at or above a `MIN_COMPATIBLE_VERSION` constant
(currently `1.28.0`) rather than an exact match against the running
version — so an ordinary app update doesn't discard an otherwise
still-valid cache, and `MIN_COMPATIBLE_VERSION` only needs bumping when a
release actually changes the cache entry layout or the AST shape it
embeds. Any mismatch, or any I/O/(de)serialization failure reading the
cache, falls back to a fresh parse, so a stale or corrupt cache never
surfaces as a lint error. Since it lives in `papyrus-lint-core`, the same
cache backs the editor extensions too, which invoke `PapyrusLinterCLI` as
a subprocess. The on-disk entry also carries a `tokens` field alongside
`ast` (both `Option`s, so writing one preserves the other's still-valid
cached value via a read-modify-write against the existing entry), for the
lexer's own token stream (`papyrus_parser::tokenize()`'s output), cached
via `get_tokens`/`put_tokens` the same way `get`/`put` cache the AST;
`put_tokens` is also called directly (alongside `put`) wherever
`parse_psc_file` and `function_table.rs`'s cross-script lookups freshly
parse a script, independent of `ast_cache::ensure_primed` below. The
on-disk entry format (the `modified_unix_secs`/`content_md5`/
`linter_version`/`ast`/`tokens` envelope, and the `ast`/`tokens` fields'
own shape) is published as a [JSON
Schema](docs/ast-cache-entry.schema.json) using JSON Schema Draft
2020-12, versioned the same way the cache itself is: it describes
entries whose `linter_version` is at or above `MIN_COMPATIBLE_VERSION`,
so a consuming tool should check a read entry's `linter_version` against
its own known-compatible floor the same way before trusting this schema,
and bump that floor whenever `MIN_COMPATIBLE_VERSION` moves. Update it
alongside any change to `CacheEntry` or to `papyrus_parser::ast::Script`
that bumps `MIN_COMPATIBLE_VERSION`.

Since `papyrus_lints::lint()`/`repair()` parse and tokenize their `source`
argument internally and never see a file path, they can't consult
`ast_cache` directly by themselves. `ast_cache::get`/`get_tokens` close
that gap as a side effect: a disk cache hit for either also primes
`papyrus-parser`'s own in-memory memoization (`papyrus_parser::prime_cache`/
`prime_tokenize_cache`, see below) with the same value, so anything that
parses/tokenizes that exact source text later in the same process --
including a lint/repair pass's own internal `parse()`/`tokenize()` calls --
reuses it instead of redoing the work. `ast_cache::ensure_primed` wraps
both accessors for the "about to lint/repair a script whose disk cache
might already be current" case: a disk cache hit for either primes the
matching in-memory cache as above; a miss for either parses/tokenizes the
source once itself (which populates the in-memory cache the same way a hit
would) and writes a fresh disk entry for next time. It's what the app's
`lint_psc_file`/`repair_psc_file`/`repair_psc_finding`/`repair_psc_file_rule`
commands and the CLI's own per-script lint loop call before linting/
repairing, so relinting an unchanged script -- across separate desktop app
commands or CLI invocations, not just the narrower `parse_psc_file`/
cross-script-lookup cases above -- skips both re-parsing and re-tokenizing
it too.

`papyrus-parser`'s own `parse()` and `tokenize()` entry points
(`app/crates/papyrus-parser/src/cache.rs`) are memoized in-memory against
the most recently seen source string: a single
`papyrus_lints::lint()`/`lint_with_external_arguments()` pass over one
script calls into them dozens of times (each AST-based lint rule calls
`parse()`, each rule that works on raw tokens instead — e.g.
`chain_whitespace`, `exclamation_spacing`, `indentation` — calls
`tokenize()`) with the exact same source text, so a single-slot,
thread-local cache keyed by content equality turns all but the first call
into a clone instead of a re-lex/re-parse. This is deliberately simpler
than the disk-backed `ast_cache`: it never outlives the process (or even
the thread) and so needs no path, mtime, or version bookkeeping, since it
only ever has to remember the one source string a lint/repair pass is
currently working on. `papyrus_parser::prime_cache`/`prime_tokenize_cache`
(`cache.rs`'s `prime`/`prime_tokens`) are the two ways this in-memory
cache is seeded from outside the crate, letting a caller that already has
a validated AST/token stream for that same source text -- namely
`ast_cache::get`/`get_tokens`, above -- insert it directly instead of
leaving the pass's first `parse()`/`tokenize()` call to compute it.

The desktop app's code viewer has a "Preview fixes" button next to its
whole-file "Apply fixes" button, the GUI counterpart of the CLI's
`fix --dry-run`: `app/src/main.ts`'s `handleCodeViewerPreviewFixClick`/
`previewRepairPscFile` call the `preview_repair_psc_file` Tauri command
(`app/src-tauri/src/lib.rs`), which computes the same whole-file repair
(`papyrus_lints::repair`) `repair_psc_file` applies but never writes it to
disk, returning a standard unified diff (empty when nothing would change)
instead. `renderDiffOutput` shows that diff in a `<pre id="code-viewer-
diff-output">` panel beneath the viewer, coloring added/removed/context/
header lines via their own `code-viewer__diff-line--*` class the way a
typical diff viewer does. The diff renderer itself
(`unified_diff`) lives in `papyrus_lint_core::diff` — moved there from
`papyrus-lint-cli`'s own formerly-private `diff` module, which now imports
it from there instead — so the CLI's `fix --dry-run` and the desktop app's
`preview_repair_psc_file` command share the exact same diff output. The
"Preview fixes" button shares "Apply fixes"'s visibility rule (view mode
only, at least one fixable finding remaining) via
`updateCodeViewerFixButtonsVisibility`; the shown preview itself is
cleared again on switching to Edit or once a real "Apply fixes" run
actually changes the file, since a stale preview would no longer be
accurate at that point.

Every line rendered by the code viewer's view-mode table also gets its own
"Fix"/"Ignore" buttons in a third `code-viewer__line-actions` cell,
generated by `buildLineActionsHtml` alongside the rest of that line's HTML
in `renderCodeViewerView`: "Fix" only when at least one of that line's
findings is auto-fixable (`isFixableFinding`), "Ignore" whenever at least
one carries a rule id at all (a rule-less finding, e.g. a compiler
diagnostic from `compile_diagnostics.rs`, can't be named in a disable
comment). Since that HTML is rebuilt from a string on every render rather
than built up as individual DOM nodes, both buttons are wired through one
delegated `click` listener on `codeViewerViewEl`
(`handleCodeViewerLineActionClick`), reading which line and which action a
click landed on off the clicked button's own `data-line`/`data-line-action`
attributes rather than each needing its own listener. "Fix"
(`handleCodeViewerFixLineClick`) collects the distinct fixable rule ids
found on that line and applies each one's fix restricted to that line via
`repairPscFinding` in turn — the same per-finding "Fix this issue" repair,
just looped once per rule instead of once per finding — logging and
skipping a rule whose fix would shift other lines (e.g. `property-sorting`)
rather than letting it block the rest. "Ignore"
(`handleCodeViewerIgnoreLineClick`) instead collects every rule id found on
the line and calls the `add_disable_comment_to_psc_line` Tauri command
(`app/src-tauri/src/lib.rs`), which wraps the new
`papyrus_lints::add_disable_comment(source, line, rules)` — a thin public
wrapper around `disable_comments::add_disable_directive`, the crate-private
module that already parses `; @disable`/`; @disable-file` comments (see
Disabling a lint on a specific line in the README) — to add or extend an
`; @disable <rule-id>[, ...]` comment on that line instead of fixing it.
`add_disable_directive` merges into an already-present `@disable` directive
on that line (leaving a bare, already-everything-suppressing `@disable`
untouched) rather than appending a second `@disable`, since
`disable_comments::parse_directive` only ever recognizes the first
occurrence in a line's comment. Both buttons re-read the file and
re-render the viewer/Lint results list entry afterward the same way
`handleCodeViewerFixClick` does, so acting on a single line, like acting on
the whole file, never requires closing the viewer first.


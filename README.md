# Papyrus Lint [![Quality gate status](https://sonarcloud.io/api/project_badges/measure?project=Idrinth_papyrus-lint&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=Idrinth_papyrus-lint) [![Discord Server](https://img.shields.io/badge/discord-server-5865F2?logo=discord)](link=https://discord.gg/idrinth) [![NexusMods](https://img.shields.io/badge/nexusmods-page-yellow)](https://www.nexusmods.com/skyrimspecialedition/mods/189862) [![GitHub](https://img.shields.io/badge/github-repo-white?logo=github)](https://github.com/idrinth/papyrus-lint) [![Action](https://img.shields.io/badge/GitHubAction-Ready-Purple?logo=GitHub&label=Action&color=purple)](https://github.com/marketplace/actions/papyrus-lint) [![Feedback](https://img.shields.io/badge/feedback-tally-orange)](https://tally.so/r/aQL1dB)

![Papyrus Lint logo](shared/images/logo-small.jpg)

**Papyrus Lint goes far beyond style: it catches bugs that CreationKit's
compiler lets through.**
`PapyrusCompiler.exe` only checks that a script is syntactically valid — it
will happily compile a script that dereferences a `None` object at
runtime, passes a `String` where an `Int` is expected, returns the wrong
type from a function, compares an `Int` to a `Float` inexactly, or
contains a branch or statement that can never execute. Those bugs don't
show up until later, as a CTD, a quest stage that never advances, or a
value that's silently wrong — often far from the line that actually
caused them, and long after the mod author has lost the context to spot
it. Papyrus Lint performs deep correctness, reliability, and performance
checks across your `.psc` source (see the full list below), on top of the
formatting and style checks a linter usually provides, so a mod author
finds these problems at write time instead of from a bug report.

## Simple Example

```papyrus
Function DoSomething(Actor a, Actor b, Form SomeItem)
    a.RemoveItem(SomeItem, 1, b);
EndFunction
```

This compiles, but the likely desired version is:

```papyrus
Function DoSomething(Actor a, Actor b, Form SomeItem)
    a.RemoveItem(SomeItem, 1, false, b);
EndFunction
```

If you don't want this to turn up, because you actually meant what you wrote:

```papyrus
Function DoSomething(Actor a, Actor b, Form SomeItem)
    a.RemoveItem(SomeItem, 1, b); @disable 
EndFunction
```

The strict boolean check identifies this issue because the first call passes
the `Actor` value `b` to a `Bool` parameter. This specific mistake cost me a
good two hours finding manually in one of my mods.

![Papyrus Lint Import](shared/images/papyrus-lint-import.png)

Just drop your file or archlist here and see the results.

See [more examples](docs/examples.md) of bugs Papyrus Lint catches that
`PapyrusCompiler.exe` lets through.

## What this is NOT

- A compiler (this uses the standard papyrus compile under the hood)
- An editor(see VSCode or Sublime Text for editors supported by our plugins)
- A guarantee a script is fit for purpose
- A replacement for proper testing
- An AI or AI-powered

![Papyrus Lint VSCode Extension](shared/images/papyrus-lint-vscode.png)

## What is a linter?

A linter is a tool that scans source code for patterns that are likely to be
mistakes, bad practice, or inconsistent style, without actually running the
code. It works from heuristics — recognizable patterns known to often cause
problems — rather than proving that a given line is definitely wrong.
Because of that, a linter can produce **false positives**: diagnostics on
code that is actually fine, especially for patterns the checks intentionally
can't fully resolve (see e.g. "Strict boolean check" or "None used as an
existing Form" below, which skip anything they can't determine with
confidence rather than guess). It's normal to disagree with an individual
diagnostic and dismiss it.

Treat every diagnostic here as **advice, not a guaranteed defect report**: a
suggestion worth a second look, not proof the code is broken. Use your own
judgment for whether a flagged line needs changing, and use the
[`; @disable`](#disabling-a-lint-on-a-specific-line) comment below to
silence a specific rule on a specific line (or [`; @disable-file`](#disabling-a-lint-on-a-specific-line)
to silence it across the whole file) when you've decided it doesn't apply.

![Papyrus Lint Results](shared/images/papyrus-lint-results.png)

## Implemented Lints

![Papyrus Lint Viewer](shared/images/papyrus-lint-viewer.png)

Every rule works from raw source or lexer tokens rather than requiring a
script that parses cleanly. For the full, searchable reference — every
rule's id, severity, tags, auto-fix support, and complete documented
behavior — see the [lint rule reference](https://papyrus-lint.idrinth.de/rules.html)
on the project website.

### Formatting

These lints make scripts easier to read by enforcing a consistent look
and feel. They are especially useful in group projects.

### Performance

To keep scripts running smoothly, this flags calls and patterns known to
run slower than necessary, most useful for anything running in a hot loop
or on a frequent update.

### Reliability

To catch mistakes that still compile fine but can misbehave once the game
is actually running, this checks cross-script calls, states, and type
usage for problems the compiler itself doesn't flag.

### Bugprone

To catch mistakes that compile clean but crash or silently do the wrong
thing at runtime, this flags patterns that are almost always bugs rather
than intentional code.

### Other

Everything that doesn't fit the categories above, from unused code and
excessive complexity to naming and configuration-driven style
preferences.

The formatting lints/fixes (trailing whitespace, space after comma,
semicolon, indentation, chain whitespace, exclamation mark spacing,
operator spacing, and assignment operator spacing) never flag or change a
line inside a
CreationKit-generated `;BEGIN FRAGMENT CODE`/`;END FRAGMENT CODE` block,
except the actual script code between a `;BEGIN CODE`/`;END CODE` pair
within it. Reformatting the rest of that block (fragment headers, the
generated function signature, `EndFunction`, or the markers themselves)
would make CreationKit fail to recognize the fragment.

![Papyrus Lint Mass Fix](shared/images/papyrus-lint-massfix.png)

## Disabling a lint on a specific line

A line carrying a trailing `; @disable <rule-id>[, <rule-id>...]` comment
has diagnostics from the named rule(s) suppressed for that line only, e.g.:

```papyrus
action = 1 ; @disable float-to-int
```

The desktop app's code viewer can add this comment for you instead of
typing it by hand — see its per-line "Ignore" button
[above](#fixing-lint-findings).

`; @disable` with no rule ids suppresses every lint on that line. Matching
against the directive's rule id(s) is case-insensitive. This only affects
linting — it does not change what automatic fixes do to that line. A
rule's id is named on its own row on the [lint rule
reference](https://papyrus-lint.idrinth.de/rules.html) (e.g. `float-to-int`
for "Implicit Float-to-Int conversion", linked directly at
[`#rule-float-to-int`](https://papyrus-lint.idrinth.de/rules.html#rule-float-to-int)).

A `; @disable-file <rule-id>[, <rule-id>...]` comment does the same across
the entire file instead of just the line it's written on, no matter where
in the file it appears, e.g.:

```papyrus
; @disable-file float-to-int
```

`; @disable-file` with no rule ids suppresses every lint in the file. It
accepts the same rule ids, matched the same case-insensitive way, and
likewise never changes what automatic fixes do. The **Unused disable
directive** lint (`unused-disable`) treats an `@disable-file` directive the
same way it treats `@disable`: an unknown rule id, or one that never
produces a diagnostic anywhere in the file, is flagged; a bare
`@disable-file` is flagged only when the whole file has no diagnostics at all.

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed at the project root:
next to the `.achlist` file you drop into the app, or, for a single
`.psc` file dropped directly, two directories above it (e.g. `Data` for
`Data/Scripts/Source/abc.psc`). Any key it omits falls back to its
default. The full default configuration, with every key documented inline,
is checked in at
[`docs/papyrus-lint.default.yaml`](docs/papyrus-lint.default.yaml) — it's
also what `PapyrusLinterCLI init` (or `init --preset strict`, the default)
writes into a project with no config file yet.

`PapyrusLinterCLI init --preset <name>` picks a different built-in starting
point instead: `standard` keeps every rule with a real correctness/
performance stake plus the cheap, auto-fixable formatting rules, and turns
off purely naming/style and informational/advisory rules; `careful` keeps
only `medium`/`high` importance rules and relaxes the cyclomatic complexity
thresholds, for a quiet first pass over an unfamiliar or legacy codebase.
See [`docs/presets/`](docs/presets/) for each built-in preset's own
annotated YAML.

Besides the three built-ins, `--preset <name>` also accepts the name of a
user preset: place a `<name>.yaml` (or `.yml`) file in a `presets`
directory next to the running executable (the CLI binary, or the desktop
app's binary when it delegates to CLI mode) and it becomes selectable the
same way, e.g. a `presets/my-team.yaml` next to the binary is picked with
`init --preset my-team`. This is separate from the executable-adjacent base
config described below: a user preset is a full baseline `init` starts
from, while the base config always layers on top of whichever preset
(built-in or user) is selected.

`PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml>` adds a
user preset the same way, without placing the file under the `presets`
directory by hand: it copies the config at the given path into that
directory as `<name>.yaml`, creating the directory first if it doesn't
exist yet. `<name>` can't be blank or match a built-in preset name
(`strict`, `standard`, `careful`), since a preset by one of those names
could never actually be selected. If a preset named `<name>` already
exists, it's left untouched and an error is reported unless `--yes` is
also given — the confirmation an overwrite requires, since the CLI has no
interactive prompt.

The desktop app's Settings tab has a "Configuration file" field for
overriding this auto-detection: enter the path to a specific
`papyrus-lint.yaml`/`.yml` file (it need not be named that, or live at the
project root) and the app reads/writes lint settings there instead,
regardless of which project directory is currently loaded — useful for
switching between several saved configurations, or for a project whose
config file doesn't live where auto-detection expects it. Leave it blank
to go back to auto-detection. Whatever path is entered is remembered
across app restarts, so it's prefilled the next time the app opens.

The desktop app offers the same presets — the three built-ins plus any user
preset found under the executable-adjacent `presets` directory — as its own
first-run picker: the first time it opens a project directory with no
`papyrus-lint.yaml`/`.yml` of its own yet (and no "Configuration file"
override set), it asks which preset to start from instead of silently
linting against the engine's defaults. Every setting a preset picks can
still be changed afterward in the Settings tab. Closing the dialog without
choosing one leaves the project on the engine's built-in defaults without
writing a config file, so it's asked again next time that directory is
opened.

A "Save current settings as preset…" button at the bottom of the Settings
tab goes the other way: it saves whatever the tab is currently set to as a
new user preset, in the same executable-adjacent `presets` directory, so it
becomes selectable from that first-run picker (or the CLI's `--preset
<name>`) immediately afterward. It asks for a name and, if a preset already
exists under it (a built-in name is rejected outright, since `--preset`
always resolves those first), asks to overwrite it before replacing it.

A "Presets" tab appears next to Settings once at least one user preset
exists (built-in presets can't be edited, so the tab stays hidden without
one), listing each with Rename, Export, and Delete buttons: Rename asks for
a new name with the same overwrite confirmation as saving one; Export
downloads its `papyrus-lint.yaml` content as-is; Delete asks to confirm and
removes it from the `presets` directory.

Each key:

- `compiler_path`: an explicit path to `PapyrusCompiler.exe`, set via the
  app's Settings tab. When unset (or blank), the app auto-detects it at
  `PapyrusCompiler.exe` inside a `Papyrus Compiler` directory one level
  above the project's `.achlist` directory (the layout used by Bethesda's
  Creation Kit tooling, where a game's `Data` directory sits alongside a
  `Papyrus Compiler` directory in the game's install root).
- `additional_script_roots`: extra directories, besides the conventional
  `scripts/source` and `source/scripts` under the project root, to search
  for `.psc` files — set via the app's Settings tab, one per line. Each
  entry is resolved relative to the project root unless it's already an
  absolute path. Searched (after the two conventional directories, in the
  order listed) when resolving cross-script lookups for the "Argument type
  check"/"Return type check"/"Function override" lints and autocompletion,
  and appended to the compiler's `-i` argument (see Compiling a script
  below) — useful when a script imports from a shared library location
  outside the project. The CLI also accepts one or more `--script-root
  <path>` flags on top of this setting (see Command-line interface below).
- `lookup_script_roots`: extra directories searched only as a last-resort
  fallback when resolving a script by name for analysis — argument/return
  types, `Extends`, autocompletion — set via the app's Settings tab, one
  per line. Each entry is resolved relative to the project root unless
  it's already an absolute path. They are searched only after the two
  conventional directories and `additional_script_roots` above, are never
  linted themselves, are ignored by `conflicting-script-versions`, and are
  not added to the compiler's `-i` argument. Intended for the game's own
  vanilla sources so a project can type-check against them without treating
  them as part of the project. Creating a new config (`init`, or the
  desktop app's first-run preset picker) or updating an existing config
  that does not yet set this key fills Skyrim Special Edition's
  `Data/Scripts/Source` and `Data/Source/Scripts` when those directories
  exist and the install path can be read from the Windows registry
  (`HKLM\Software\Bethesda Softworks\Skyrim Special Edition` or
  `HKLM\Software\Wow6432Node\Bethesda Softworks\Skyrim Special Edition`,
  value `installed path`). An explicit empty list is left empty rather
  than re-filled.
- `compile_check`: whether the desktop app and the CLI also run
  PapyrusCompiler.exe against a `.psc` as part of linting it — set via the
  app's Settings tab, alongside `compiler_path`. `false` by default, since
  it's slower than the lint engine's own, dependency-free checks and
  requires a compiler path — configured or auto-detected (see
  `compiler_path` above). When enabled, PapyrusCompiler.exe's
  own reported errors (e.g. a syntax mistake the lint engine's more
  forgiving parser lets through) are added to the results as `[error]`
  diagnostics, the same way the app's other lints are. Compiles into a
  throwaway temporary directory rather than the project's real output
  directory, so enabling this never touches (or requires write access to)
  the project's actual compiled `.pex` output — see Compiling a script
  below.
- `strict_achlist_scope`: `false` by default. When an `.achlist`'s entries
  live in arbitrary, non-conventional source directories, the CLI needs
  some way to let those entries resolve each other for the "Argument type
  check"/"Return type check"/"Function override" lints and
  `conflicting_script_versions`. By default, it does this by adding every
  listed entry's own directory as an additional search root — which means
  a script in that directory that *isn't* itself listed in the achlist can
  still resolve, and `conflicting_script_versions` still scans that whole
  directory, not just the achlist's other listed entries. Setting this to
  `true` instead resolves strictly among the achlist's own listed entries,
  without treating their directories as search roots at all: it never lets
  an unlisted file resolve just because it happens to sit alongside a
  listed one, and is dramatically faster on an achlist whose entries are
  spread across many directories (see
  [#311](https://github.com/Idrinth/papyrus-lint/issues/311)) — but every
  `.psc` a listed entry depends on has to be listed in the achlist itself.
- `semicolon`: whether lines are required to end in a semicolon (`true`)
  or must not (`false`). Read by the "Semicolon at end of line" lint/fix.
- `indentation`: the expected indentation style, `tab` or `space`. Read
  by the "Formatting checks" lint and the indentation automatic fix.
- `indentation_width`: the number of spaces per indentation level, used
  only when `indentation` is `space`.
- `identifier_casing`: the casing style declared identifiers must match:
  `camelCase`, `PascalCase`, `snake_case`, or `CONSTANT_CASE`. Read by the
  "Identifier casing" lint.
- `cyclomatic_complexity_warning` / `cyclomatic_complexity_error`: the
  cyclomatic complexity a function/event can reach before the
  "Cyclomatic complexity" lint flags it as a `[warning]` or an `[error]`,
  respectively. `cyclomatic_complexity_error` set below
  `cyclomatic_complexity_warning` is treated as equal to it.
- `type_casing`: the casing convention required of a script's declared
  type name (the identifier following `ScriptName`), one of `PascalCase`,
  `camelCase`, `lowercase`, or `UPPERCASE`. Read by the "Type name casing"
  lint.
- `named_arguments`: how strongly a positional call argument should be
  passed by name instead, one of `always`, `instead_of_defaults`, or
  `never` (the default). Read by the "Prefer named arguments" lint.
- `min_wait_interval`: the interval/duration argument a `Utility.Wait`,
  `RegisterForUpdate`, `RegisterForSingleUpdate`,
  `RegisterForUpdateGameTime`, or `RegisterForSingleUpdateGameTime` call
  can go below before the "Short wait/update interval" lint flags it as a
  `[warning]`. Defaults to `0.1`.
- `magic_numbers`: whether the "Magic numbers" lint also flags a
  `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/
  `RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call's
  interval argument, `loose` (the default) or `strict`.
- `fail_on_warning` / `fail_on_info`: whether the command-line interface
  (see below) treats a `[warning]`-level or `[info]`-level diagnostic,
  respectively, as a reason to exit non-zero. Both default to `false`, so
  by default only `[error]`-level diagnostics fail a CLI run;
  `[warning]`/`[info]`-level diagnostics are still printed either way. Has
  no effect on the desktop app, which always lists every diagnostic
  regardless of severity.
- `bool_like_int`: whether the "Strict boolean check" lint accepts the
  `Int` literal `1` or `0` used directly as an `If`/`ElseIf`/`While`
  condition, treating it as the common "bool-like" idiom instead of
  flagging it. `true` by default; any other `Int` value (a variable, a
  property, or a literal other than `1`/`0`) is still flagged regardless
  of this setting.
- `assume_auto_properties_filled`: whether the "None used as an existing
  Form" lint treats a script-level `Auto`/`AutoReadOnly` property as
  already filled in by the time a function runs, rather than possibly
  still `None`. `false` by default, so such a property is treated the same
  as an uninitialized local unless proven otherwise; many projects
  consider that noise once they trust their Property Manager setup, and
  can set this to `true` to drop the assumption for properties (a local
  variable is still tracked either way).
- `rules`: per-lint enable/disable switches, one key per rule (the key name
  is a rule's id with hyphens replaced by underscores, e.g. `float-equality`
  is `float_equality`). Setting one to `false` turns that lint (and its
  automatic fix, if it has one) off entirely; every key under `rules` can be
  omitted individually and falls back to its default. Every key defaults to
  `true` except `property_sorting`,
  `unchecked_form_parameter`, `unchecked_array_element`, `unused_disable`, `magic_numbers`,
  `native_function_usage`, `repeated_getvalue`,
  `global_variable_setvalue`, `default_property_value`,
  `unknown_actor_value`, `missing_doc_comment`, `float_equality`,
  `missing_update_handler`, `event_signature_mismatch`, and
  `circular_dependency`, which default to
  `false`: reordering a script's declared properties is a more invasive
  change than the rest of these lints, many scripts intentionally accept a
  possibly-`None` Form and defer the check to a caller or a later branch
  (the same reasoning extended to array elements),
  reporting stale suppressions is opt-in to avoid surprising existing
  projects, flagging every literal number in an existing script all at once
  is likely to be noisy until a project is ready for it, plenty of mods
  intentionally depend on SKSE/F4SE or another native extension and don't
  need to be warned about it, a chain that reads the same global more than
  once is often written that way deliberately for readability, the
  `GlobalVariable` no-op write lint's `Else`-branch case is a heuristic
  rather than a proven no-op, many existing scripts already rely on
  Papyrus's implicit per-type defaults for some or all of their properties,
  a project's own plugin can define additional, custom Actor Values
  that have no way to appear in `rules/actor-values.yaml`, most
  existing scripts have no documentation comments at all, so flagging
  every declaration missing one would be noisy until a project opts in,
  a project may deliberately compare two `Float` values it knows are
  computed the exact same way, so flagging every such comparison by default
  would be a false positive, the "Missing update event handler" lint only
  ever sees a single script's own source, so a matching `Event` declared
  instead on a script it `Extends` would otherwise be misreported as
  missing, the "Event signature
  mismatch" lint's `rules/known-events.yaml` only lists a curated
  subset of the engine's native events and matches an `Event`'s name alone,
  regardless of whether the enclosing script actually extends the Form
  that declares it, and two scripts intentionally holding `Property`
  references to each other for two-way communication (e.g. a manager and a
  worker script) is a common, legitimate design that enabling the "Circular
  script dependency" lint by default would flag as a mistake.
  See [`docs/papyrus-lint.default.yaml`](docs/papyrus-lint.default.yaml) for
  every rule's key name and default value together in one place.

The app's formatting controls (trailing semicolons, indentation style,
indentation width) are backed by this file: on startup it reads the
config file for the most recently opened project and pre-selects those
controls accordingly, and any change made to them is written straight
back to the file, so the project's formatting settings persist between
sessions and can be shared/committed alongside the project. The
PapyrusCompiler.exe path field on the Settings tab works the same way,
except it's pre-filled with an auto-detected path (see `compiler_path`
above) rather than a fixed default when the project has no explicit
override saved yet. The additional script roots textarea (see
`additional_script_roots` above) and the lookup script roots textarea
(see `lookup_script_roots` above) work the same way too, one directory
per line.

## Command-line interface

![Papyrus Lint CLI example](shared/images/papyrus-lint-cli.png)

Besides its GUI, Papyrus Lint can lint non-interactively from the
command line two ways: by passing an `.achlist` (or a single `.psc`, or a
directory) path to the desktop app's own executable (`PapyrusLinter`), or
via the standalone `PapyrusLinterCLI` binary (`app/crates/papyrus-lint-cli`)
built and shipped separately for use cases — e.g. a CI pipeline — that
shouldn't need the desktop app's binary (and its GUI dependencies) at all.
Both accept the same argument and behave identically:

```text
PapyrusLinterCLI path/to/project.achlist
PapyrusLinterCLI init
PapyrusLinterCLI init --preset standard
PapyrusLinterCLI init --preset my-team
PapyrusLinterCLI preset add my-team path/to/papyrus-lint.yaml
PapyrusLinterCLI preset add my-team path/to/papyrus-lint.yaml --yes
PapyrusLinterCLI doctor path/to/project.achlist
PapyrusLinterCLI doctor --json path/to/project.achlist
PapyrusLinterCLI path/to/Example.psc
PapyrusLinterCLI path/to/scripts/source
PapyrusLinterCLI fix path/to/project.achlist
PapyrusLinterCLI fix path/to/Example.psc
PapyrusLinterCLI fix --type trailing-whitespace path/to/Example.psc
PapyrusLinterCLI fix --line 12 --type trailing-whitespace path/to/Example.psc
PapyrusLinterCLI fix --dry-run path/to/project.achlist
PapyrusLinterCLI --tag style path/to/project.achlist
PapyrusLinterCLI fix --tag style path/to/project.achlist
PapyrusLinterCLI --json path/to/project.achlist
PapyrusLinterCLI --format ai path/to/project.achlist
PapyrusLinterCLI --format ai --hash-source path/to/project.achlist
PapyrusLinterCLI --json fix path/to/project.achlist
PapyrusLinterCLI --config path/to/papyrus-lint.yaml path/to/Example.psc
PapyrusLinterCLI --script-root path/to/SharedScripts path/to/project.achlist
PapyrusLinterCLI --output path/to/report.txt path/to/project.achlist
PapyrusLinterCLI --json --output path/to/report.json path/to/project.achlist
PapyrusLinterCLI --short-paths path/to/project.achlist
PapyrusLinterCLI --color never path/to/project.achlist
PapyrusLinterCLI --progress --output path/to/report.txt path/to/project.achlist
PapyrusLinterCLI --threads 8 path/to/project.achlist
PapyrusLinterCLI --threads 1 path/to/project.achlist
PapyrusLinterCLI --blob "ScriptName Example extends ObjectReference"
PapyrusLinterCLI --json --blob "ScriptName Example extends ObjectReference"
PapyrusLinterCLI --config path/to/papyrus-lint.yaml --blob "ScriptName Example extends ObjectReference"
```

`PapyrusLinterCLI init` creates a `papyrus-lint.yaml` in the current working
directory from the selected `--preset` (`strict`, `standard`, or `careful`,
matched case-insensitively; defaults to `strict`, identical to today's
built-in default — see Configuration above and
[`docs/presets/`](docs/presets/)). Any other `--preset` name is looked up
as `<name>.yaml`/`.yml` (matched case-insensitively) in a `presets`
directory next to the running executable (see Configuration above). It
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
selectable via `init --preset <name>` afterward (see Configuration above).
It refuses a blank name or one matching a built-in preset (`strict`,
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

Given an `.achlist` path, it resolves every `.psc` entry listed in it.
Given a single `.psc` path directly, it lints just that file, treating it
as the achlist's sole entry. Given a directory instead, it recursively
scans it (and every subdirectory beneath it, at any depth) for `.psc`
files and lints every one found — useful for a mod whose scripts are
spread across arbitrarily nested subfolders instead of a flat
`scripts/source` (e.g. Requiem's own layout) and that ships no `.achlist`
at all. Either way, each script is linted against the project's
`papyrus-lint.yaml`/`.yml` config file (see Configuration above). For an
`.achlist`, the project root is its containing directory; for a bare
`.psc`, the root is found by walking up from the file for a
`Scripts/Source` or `Source/Scripts` directory pair (matched
case-insensitively) and taking the directory above it — so it's found
correctly even for a script nested further still, e.g. a namespaced
`Scripts/Source/User/MyScript.psc`, not just the conventional two
directories up. A scanned directory's own resolved scripts are tried the
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
the project's configured `additional_script_roots` (see Configuration
above) — letting a caller add a script root for a single run without
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
[JSON Schema](docs/papyrus-lint-report.schema.json) using JSON Schema Draft 2020-12,
so integrations can generate types and validate saved or streamed reports:

```console
PapyrusLinterCLI --json --output report.json path/to/project.achlist
npx ajv-cli validate --spec=draft2020 -s docs/papyrus-lint-report.schema.json -d report.json
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
for a rule with
no known tag metadata (e.g. a compiler-reported diagnostic). `files_fixed` is
only present (non-`null`) when run with the `fix` subcommand, and under
`fix --dry-run` counts scripts that *would* have been fixed rather than
scripts actually rewritten on disk. `dry_run` reports whether this was a
`fix --dry-run` run; each file's `diff` is only non-`null` in that case,
for a script that would actually have changed, and carries the standard
unified diff between its original and would-be-fixed source. `success`
reports whether the run would exit `0`; here it's `true` because a
`[warning]`-level diagnostic doesn't fail the run under the default
`fail_on_warning: false`.

It exits `0` if no diagnostics were found (or none of the ones found
counted as a failure — see `fail_on_warning`/`fail_on_info` under
Configuration above), `1` if any did, or `2` on a usage error (a
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

Prebuilt `PapyrusLinterCLI`/`PapyrusLinterCLI.exe` standalone CLI binaries
for Linux, macOS, and Windows are attached to each [GitHub
release](https://github.com/Idrinth/papyrus-lint/releases), alongside the
desktop app's own bundles. To build the standalone CLI yourself instead,
run `cargo build --release --manifest-path
app/crates/papyrus-lint-cli/Cargo.toml`; the resulting binary is named
`PapyrusLinterCLI`.

### Docker

Each release also publishes an Alpine Linux CLI image to GitHub Container
Registry. It always scans `/project` recursively, stores the reusable AST
cache in `/cache`, and uses Papyrus base scripts mounted under
`/base-scripts` for cross-script lookups. The base scripts can be an extracted
directory or a zip named `skyrim-scripts.zip`; set
`PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE` when the mounted archive has another name.

```bash
docker run --rm \
  -v "$PWD:/project:ro" \
  -v papyrus-lint-cache:/cache \
  -v "$HOME/skyrim-scripts.zip:/base-scripts/skyrim-scripts.zip:ro" \
  ghcr.io/idrinth/papyrus-lint:latest
```

Additional CLI flags go after the image name. For example, append `--json`
for JSON output. Omit `:ro` from the project mount and pass `fix` if the
container should apply automatic fixes. A release-specific image tag such as
`v1.2.3` can be used instead of `latest`. Container images are signed keylessly
by the release workflow. Verify a release tag with `cosign` (replace the tag as
needed):

```bash
cosign verify \
  --certificate-identity \
    https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v1.2.3 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  ghcr.io/idrinth/papyrus-lint:v1.2.3
```

Every file attached to a release has a matching `.sigstore.json` bundle.
The release workflow signs these bundles keylessly with [Sigstore](https://www.sigstore.dev/),
using GitHub Actions' short-lived OpenID Connect identity, and records the
signature in Sigstore's transparency log. After installing `cosign`, verify a
download by keeping it next to its bundle and running (replace the file name
and tag as needed):

```bash
cosign verify-blob \
  --bundle PapyrusLinterCLI-linux.sigstore.json \
  --certificate-identity \
    https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v1.2.3 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  PapyrusLinterCLI-linux
```

This verifies both that the downloaded bytes have not changed and that they
were signed by this repository's tag-triggered release workflow. The signing
key is ephemeral, so there is no long-lived release-signing secret to rotate
or expose.

The preferred way to run Papyrus Lint in CI is the [Papyrus Lint GitHub
Action](https://github.com/marketplace/actions/papyrus-lint)
(`idrinth/papyrus-lint-action`), which downloads `PapyrusLinterCLI` for you
and, on pull requests, posts findings as inline review comments on the
changed lines. See [`docs/github-actions-example.md`](docs/github-actions-example.md)
for a usage example, and for a manual workflow that downloads a release's
`PapyrusLinterCLI` binary directly instead.

Each release also attaches a packaged copy of the two editor plugins: the
VS Code extension as a `.vsix` file (install via "Install from VSIX..." in
VS Code, or `code --install-extension <file>`) and the SublimeLinter
plugin as a `.zip` of the `SublimeLinter-contrib-papyrus-lint` directory
(extract it into Sublime Text's `Packages/` directory). Both still depend
on the standalone `PapyrusLinterCLI` binary above being installed and on
`PATH` (or configured via each plugin's settings).

## Versioning

Papyrus Lint follows [Semantic Versioning](https://semver.org/). Release
versions use the `MAJOR.MINOR.PATCH` format: major releases contain breaking
changes, minor releases add backward-compatible functionality, and patch
releases contain backward-compatible fixes.

## Fixing lint findings

Each `.psc` file listed on the Lint results tab that has at least one
finding for an auto-fixable lint (marked as such on the [lint rule
reference](https://papyrus-lint.idrinth.de/rules.html)) shows an "Apply
fixes" button, applying every automatic fix to that file at once — the
same repair the CLI's `fix` subcommand applies to a single script (see
Command-line interface below). Each individual finding for an
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
save.

Every line in the code viewer that has at least one finding also gets its
own small "Fix"/"Ignore" buttons next to it, in view mode. "Fix" only
appears when at least one of that line's findings is auto-fixable, and
applies each such finding's own fix restricted to that line, the same way
"Fix this issue" does — a rule whose fix would shift other lines (e.g.
`property-sorting`, or `unused-import` removing its own `Import` line) is
silently skipped rather than blocking the rest.
"Ignore" appears whenever at least one finding on the line carries a rule
id, and adds a [`; @disable <rule-id>[, <rule-id>...]`](#disabling-a-lint-on-a-specific-line)
comment naming every rule found on that line instead of fixing it — merging
into an already-present `; @disable` comment on that line rather than
adding a second one, so clicking it again after a later run flags something
new just extends the same comment.

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
[v3](docs/papyrus-lint-ai-export.v3.schema.json), the current format
described below, and [v2](docs/papyrus-lint-ai-export.v2.schema.json) and
[v1](docs/papyrus-lint-ai-export.v1.schema.json), the frozen contracts
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
exported file already has at least one diagnostic),
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
`--hash-source` flag; see Command-line interface above), so a report can be
handed to an external AI without exposing proprietary script text while the
assistant can still tell files apart, or notice a file changed between
exports, from the hash alone; or an object with `"type": "error"` carrying
a `message` describing why the source couldn't be read (e.g. it was moved
or deleted since linting) — each diagnostic raised by PapyrusCompiler.exe
itself (rule `compiler-error`, see Compiling a script below) rather than
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
this README gives a human reader, to answer follow-up questions precisely
without needing this project's own documentation on hand. Like "Export
issues", it's disabled whenever no finding currently passes the active
filters.

## Compiling a script

Each `.psc` file listed on the Lint results tab has a "Compile" button that
recompiles it with `PapyrusCompiler.exe` (see `compiler_path` under
Configuration above for how that executable's path is resolved), so a fix
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
project root, plus any configured `additional_script_roots` (see
Configuration above), separated by `;` (PapyrusCompiler.exe accepts
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

Enabling `compile_check` (see Configuration above) also runs
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

## Thank Yous

A big thank you to WraithFallen for doing a massive testing run on the
versions of this tool, helping find bugs and improve it further with
their dedication to rooting out false positives.

Another thank you to s3ngine and wall416 over on NexusMods for spotting
bugs and reporting them in the early development of the tool.

## How to help

You can help the project by:

- Providing examples of false positives.
- Providing examples of false negatives.
- Giving feedback on the existing rules.
- Proposing new rules or adjustments to existing rules.
- Reviewing code.
- Writing code.
- Writing or suggesting tests.
- Sponsoring development.

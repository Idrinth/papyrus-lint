# Configuration reference

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed at the project root:
next to the `.achlist` file you drop into the app, or, for a single
`.psc` file dropped directly, two directories above it (e.g. `Data` for
`Data/Scripts/Source/abc.psc`). Any key it omits falls back to its
default. The full default configuration, with every key documented inline,
is checked in at
[`docs/papyrus-lint.default.yaml`](papyrus-lint.default.yaml) — it's
also what `PapyrusLinterCLI init` (or `init --preset strict`, the default)
writes into a project with no config file yet.

## Presets

`PapyrusLinterCLI init --preset <name>` picks a different built-in starting
point instead: `standard` keeps every rule with a real correctness/
performance stake plus the cheap, auto-fixable formatting rules, and turns
off purely naming/style and informational/advisory rules; `careful` keeps
only `medium`/`high` importance rules and relaxes the cyclomatic complexity
thresholds, for a quiet first pass over an unfamiliar or legacy codebase.
See [`docs/presets/`](https://github.com/idrinth/papyrus-lint/tree/the-one/docs/presets)
for each built-in preset's own annotated YAML.

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

## Desktop app configuration management

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

The app's formatting controls (trailing semicolons, indentation style,
indentation width) are backed by this file: on startup it reads the
config file for the most recently opened project and pre-selects those
controls accordingly, and any change made to them is written straight
back to the file, so the project's formatting settings persist between
sessions and can be shared/committed alongside the project. The
PapyrusCompiler.exe path field on the Settings tab works the same way,
except it's pre-filled with an auto-detected path (see `compiler_path`
below) rather than a fixed default when the project has no explicit
override saved yet. The additional script roots textarea (see
`additional_script_roots` below) and the lookup script roots textarea
(see `lookup_script_roots` below) work the same way too, one directory
per line.

## Each key

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
  and appended to the compiler's `-i` argument (see
  [Compiling a script](compiling-scripts.md)) — useful when a script
  imports from a shared library location outside the project. The CLI also
  accepts one or more `--script-root <path>` flags on top of this setting
  (see the [CLI reference](cli.md)).
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
  the project's actual compiled `.pex` output — see
  [Compiling a script](compiling-scripts.md).
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
- `fail_on_warning` / `fail_on_info`: whether the [command-line
  interface](cli.md) treats a `[warning]`-level or `[info]`-level
  diagnostic, respectively, as a reason to exit non-zero. Both default to
  `false`, so by default only `[error]`-level diagnostics fail a CLI run;
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
  See [`docs/papyrus-lint.default.yaml`](papyrus-lint.default.yaml) for
  every rule's key name and default value together in one place.

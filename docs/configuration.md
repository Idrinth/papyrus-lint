# Configuration reference

Lint/fix behavior is configured via an optional `papyrus-lint.yaml` (or
`papyrus-lint.yml`) at the project root. Any omitted setting uses its default.
The complete, annotated default file is
[`configuration/papyrus-lint.default.yaml`](../configuration/papyrus-lint.default.yaml);
it is the reference for the available keys, accepted values, and rule switches.
`PapyrusLinterCLI init` writes this file using the `strict` preset unless another
preset is requested.

For an `.achlist`, the project root is the directory containing that file. For a
single `.psc`, the CLI looks for a nearby config and the conventional
`Scripts/Source` or `Source/Scripts` layout. See
[Resolving a project](cli.md#resolving-a-project) for the exact search order.

A JSON Schema is available both at
[`schema/papyrus-lint.schema.json`](../schema/papyrus-lint.schema.json) and at
`https://papyrus-lint.idrinth.de/schema/papyrus-lint.schema.json` for editors
that support YAML schema association.

Papyrus Lint supports `skyrim`, `fallout4`, and `starfield`. Skyrim is the
default for configurations that omit `game`. The desktop app's new-project
picker currently offers Skyrim and Fallout 4; use
`PapyrusLinterCLI init --game starfield` to create a Starfield configuration.
The desktop app preserves and displays `starfield` when it opens an existing
Starfield project.

## Suppressing one diagnostic

Diagnostics that cannot be disabled in source can be suppressed with an
optional `.papyrus-lint-ignore` YAML file in the project root. Each entry names
a source file (an absolute path or one relative to the project root), a
1-indexed line, and a rule id. It suppresses only that rule finding on that
exact line:

```yaml
- file: scripts/source/dce.psc
  line: 12
  rule: trailing-whitespace
```

## Presets

`PapyrusLinterCLI init --preset <name>` selects the starting configuration:

- `strict` enables the default rule set.
- `standard` keeps rules with a correctness or performance impact, plus cheap
  auto-fixable formatting rules, while disabling purely stylistic and
  informational rules.
- `careful` keeps only medium/high-importance rules and relaxes cyclomatic
  complexity thresholds for a quieter first pass over an unfamiliar or legacy
  project.

The annotated built-ins are under
[`configuration/presets/`](https://github.com/idrinth/papyrus-lint/tree/the-one/configuration/presets).

Custom presets are full configuration files stored as `<name>.yaml` or
`<name>.yml` in a `presets` directory next to the executable. They work in both
the CLI and desktop app. Add one without copying it manually with:

```text
PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml>
```

The name cannot be blank or one of the built-in names (`strict`, `standard`,
`careful`). Existing custom presets are not overwritten unless `--yes` is
passed.

## Desktop app configuration management

The Settings tab's **Configuration file** field overrides project-based config
detection. The selected file can have any name or location, is remembered
across restarts, and is read and written regardless of the currently loaded
project. Clear the field to restore automatic detection.

When a project has no config and no override, the app asks for a preset and a
target game. Choosing a preset writes a project config; continuing without a
preset writes one only when needed to preserve a non-default game selection.
Closing the dialog leaves the built-in defaults in effect and asks again the
next time that project opens.

**Save current settings as preset…** stores the current Settings values in the
executable-adjacent `presets` directory. When custom presets exist, the
**Presets** tab can rename, export, or delete them. Built-in presets cannot be
edited or overwritten.

## Behavior not captured by the YAML comments

- Relative `additional_script_roots` and `lookup_script_roots` are resolved
  from the project root. CLI `--script-root` values and a `.ppj` file's
  `<Import>` entries supplement `additional_script_roots`; `init` can also seed
  additional roots from a `.ppj` in the directory being initialized.
- `lookup_script_roots` are consulted only after conventional and additional
  roots. Files found only there provide type and inheritance information but
  are not linted, included in compilation, or considered by
  `conflicting-script-versions`. New or updated Skyrim and Fallout 4 configs
  are seeded with detected vanilla source directories when their install paths
  are available from the Windows registry. Starfield roots must currently be
  configured explicitly.
- With `strict_achlist_scope: false`, directories containing listed `.achlist`
  entries also become lookup roots, so unlisted neighboring scripts can
  resolve. Setting it to `true` restricts resolution to listed entries and is
  substantially faster when an achlist spans many directories, but every
  dependency must then be listed. See
  [issue #311](https://github.com/Idrinth/papyrus-lint/issues/311).
- If `compiler_path` is unset, the desktop app looks for
  `PapyrusCompiler.exe` in a `Papyrus Compiler` directory beside the game's
  `Data` directory. `compile_check` uses a temporary output directory and never
  writes compiled files into the project; see
  [Compiling a script](compiling-scripts.md).
- `fail_on_warning` and `fail_on_info` affect only the CLI exit status. The
  diagnostics are still printed, and the desktop app always displays every
  severity.
- Rule keys replace hyphens in rule ids with underscores (for example,
  `float-equality` becomes `float_equality`). Disabling a rule also disables
  its automatic fix. Consult the annotated default file for the current rule
  list and defaults rather than copying them into another document.

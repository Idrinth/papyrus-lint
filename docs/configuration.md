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

- `game`: the game whose Papyrus dialect and runtime APIs the project
  targets. Accepts `skyrim`, `fallout4`, or `starfield`; omitted keys
  default to `skyrim` for compatibility with existing configuration files.
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
  (see the [CLI reference](cli.md)), and, for a `.ppj` (Papyrus Project XML)
  input, feeds that file's own `<Import>` entries in on top too. `init` also
  seeds this setting itself from a `.ppj` file found in the directory it
  initializes, if that config has none of its own yet (see the
  [CLI reference](cli.md)'s "Initializing a project" section).
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
  that does not yet set this key fills the vanilla source directories for
  the project's own `game` (above) when those directories exist and the
  matching install path can be read from the Windows registry:
  - `skyrim`: `Data/Scripts/Source` and `Data/Source/Scripts` under the
    path from `HKLM\\Software\\Bethesda Softworks\\Skyrim Special Edition` or
    `HKLM\\Software\\Wow6432Node\\Bethesda Softworks\\Skyrim Special Edition`
    (value `installed path`).
  - `fallout4`: `Data/Scripts/Source/Base` and `Data/Scripts/Source/User`
    under the path from `HKLM\\Software\\Bethesda Softworks\\Fallout4` or
    `HKLM\\Software\\Wow6432Node\\Bethesda Softworks\\Fallout4` (value
    `installed path`).
  - `starfield`: `Data/Scripts/Source`, `Data/Scripts/Source/Base`, and
    `Data/Scripts/Source/User` under the path from
    `HKLM\\Software\\Bethesda Softworks\\Starfield` or
    `HKLM\\Software\\Wow6432Node\\Bethesda Softworks\\Starfield` (value
    `installed path`).

  An explicit empty list is left empty rather than re-filled.
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
- `fail_on_warning` and `fail_on_info` affect only the CLI exit status. The
  diagnostics are still printed, and the desktop app always displays every
  severity.
- Rule keys replace hyphens in rule ids with underscores (for example,
  `float-equality` becomes `float_equality`). Disabling a rule also disables
  its automatic fix. Consult the annotated default file for the current rule
  list and defaults rather than copying them into another document.

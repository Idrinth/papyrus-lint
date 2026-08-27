# Papyrus Lint [![Quality gate status](https://sonarcloud.io/api/project_badges/measure?project=Idrinth_papyrus-lint&metric=alert_status)](https://sonarcloud.io/summary/new_code?id=Idrinth_papyrus-lint)

![Papyrus Lint logo](resources/logo.jpg)

A linter for Bethesda's papyrus language to improve code quality.

## Implemented Lints

| Lint | Description | Auto-Fix |
| --- | --- | --- |
| **Trailing whitespace** | Flags lines that end with trailing spaces or tabs. | ✓ |
| **Space after comma** | Requires whitespace after commas in argument lists. | ✓ |
| **Forbidden/discouraged function usage** | Flags calls to functions listed in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls), with a configurable severity and an explanatory message per entry. | |
| **Getter usage without saving result** | Flags standalone calls to functions whose names begin with `Get` (case-insensitively), because their return value is discarded. | |
| **Unused script properties** | Flags `Property` declarations whose name is never referenced anywhere else in the script. | |
| **Semicolon at end of line** | Requires a trailing semicolon on each non-empty line or forbids terminal semicolons, according to the selected setting. | ✓ |
| **Implicit Float-to-Int conversion** | Flags a Float value declared, assigned, returned, or passed as an argument into an Int-typed slot without an explicit `as Int` cast. | |
| **Strict boolean check** | Flags `If`/`ElseIf`/`While` conditions that aren't already a `Bool` value or expression, instead of relying on Papyrus's implicit conversion to boolean. Only conditions whose type can be determined locally (locals, parameters, properties, literals, casts, and comparison/logical expressions) are checked; a condition that depends on a function call or a member access is left unflagged rather than risk a false positive. | |
| **Argument type check** | Flags call-site arguments whose type doesn't match the callee's declared parameter type (e.g. passing a `String` where an `Int` is expected), allowing the implicit `Int`-to-`Float` widening Papyrus itself allows, as well as passing an object whose script extends (directly or transitively) the parameter's type (e.g. passing an `Armor` where a `Form` is expected). Calls to functions declared in the same script are always checked; when linting a `.psc` file dropped in the app, calls to functions declared on other scripts under the project root (e.g. `SomeProperty.DoThing(...)`) are checked too, by resolving those scripts' signatures (including through `Extends`), and the `Extends` chain of an argument's own script is likewise resolved from the project root to allow compatible subtypes. A call whose target or argument type can't be determined is skipped rather than guessed at. | |
| **Strict numeric type check** | Flags implicit comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) between an `Int` value and a `Float` value without an explicit cast making the comparison exact. Only comparisons whose operand types can be determined locally are checked. | |
| **Formatting checks** | Flags lines whose indentation doesn't match the configured style/width (`indentation`/`indentation_width`) for their nesting depth. A script whose structure can't be identified (e.g. it doesn't lex cleanly) is left unchecked rather than guessed at. | ✓ |
| **Slow function usage** | Flags calls to functions listed in `rules/slow-functions.yaml` that have a faster equivalent available, and suggests the quicker alternative. | |
| **Cyclomatic complexity** | Flags functions/events whose cyclomatic complexity (1 plus each `If`/`ElseIf` branch, `While` loop, and short-circuiting `&&`/`\|\|` operator) exceeds a configurable threshold, as a `[warning]` above `cyclomatic_complexity_warning` (default 10) or an `[error]` above `cyclomatic_complexity_error` (default 20). | |
| **Unreachable statement** | Flags statements that follow a `Return` within the same block (a function/event body, an `If`/`ElseIf`/`Else` branch, or a `While` body), since they can never execute. | |
| **Static condition** | Flags `If`/`ElseIf`/`While` conditions that fold to a constant `true` or `false` (e.g. `If true`, `If 1 == 2`, `If !false && 3 > 4`), regardless of any runtime state, as a `[warning]`. Only conditions built entirely from literals (combined with arithmetic, comparison, logical, and unary operators) are checked; one that depends on an identifier, a call, `Self`/`Parent`, a member/index access, a cast, or a `new` array is left unflagged rather than guessed at. | |
| **Unused or write-only local variables** | Flags a local variable (declared with `Type name = ...` inside a function/event) whose value is never read: either it's never referenced again at all, or it's only ever reassigned (`name = ...`) without that new value ever being read back. Reading a variable via a compound assignment (`name += ...`, etc.) or through a member/index expression built from it (`name.Foo`, `name[0]`) counts as a use. Function parameters and script properties aren't locals and are never flagged by this lint. | |

The formatting lints/fixes (trailing whitespace, space after comma,
semicolon, and indentation) never flag or change a line inside a
CreationKit-generated `;BEGIN FRAGMENT CODE`/`;END FRAGMENT CODE` block,
except the actual script code between a `;BEGIN CODE`/`;END CODE` pair
within it. Reformatting the rest of that block (fragment headers, the
generated function signature, `EndFunction`, or the markers themselves)
would make CreationKit fail to recognize the fragment.

## Disabling a lint on a specific line

A line carrying a trailing `; @disable <rule-id>[, <rule-id>...]` comment
has diagnostics from the named rule(s) suppressed for that line only, e.g.:

```papyrus
action = 1 ; @disable float-to-int
```

`; @disable` with no rule ids suppresses every lint on that line. Matching
against the directive's rule id(s) is case-insensitive. This only affects
linting — it does not change what automatic fixes do to that line. The rule ids, one per
lint listed above, are: `trailing-whitespace`, `comma-spacing`,
`forbidden-functions`, `slow-functions`, `unused-getter`, `unused-property`,
`semicolon`, `float-to-int`, `strict-boolean`, `argument-types`,
`numeric-comparison`, `indentation`, `cyclomatic-complexity`,
`unreachable-statement`, `static-condition`, and `unused-local-variable`.

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed next to the
`.achlist` file you drop into the app. Any key it omits falls back to
its default:

```yaml
compiler_path: null
semicolon: false
indentation: tab
indentation_width: 4
cyclomatic_complexity_warning: 10
cyclomatic_complexity_error: 20
rules:
  trailing_whitespace: true
  comma_spacing: true
  forbidden_functions: true
  slow_functions: true
  unused_getter: true
  unused_property: true
  semicolon: true
  float_int_conversion: true
  strict_boolean: true
  argument_types: true
  numeric_comparison: true
  indentation: true
  cyclomatic_complexity: true
  unreachable_statement: true
  static_condition: true
  unused_local_variable: true
```

- `compiler_path`: an explicit path to `PapyrusCompile.exe`, set via the
  app's Settings tab. When unset (or blank), the app auto-detects it at
  `PapyrusCompile.exe` inside a `Papyrus Compiler` directory one level
  above the project's `.achlist` directory (the layout used by Bethesda's
  Creation Kit tooling, where a game's `Data` directory sits alongside a
  `Papyrus Compiler` directory in the game's install root).
- `semicolon`: whether lines are required to end in a semicolon (`true`)
  or must not (`false`). Read by the "Semicolon at end of line" lint/fix.
- `indentation`: the expected indentation style, `tab` or `space`. Read
  by the "Formatting checks" lint and the indentation automatic fix.
- `indentation_width`: the number of spaces per indentation level, used
  only when `indentation` is `space`.
- `cyclomatic_complexity_warning` / `cyclomatic_complexity_error`: the
  cyclomatic complexity a function/event can reach before the
  "Cyclomatic complexity" lint flags it as a `[warning]` or an `[error]`,
  respectively.
- `rules`: per-lint enable/disable switches, each defaulting to `true`.
  Setting one to `false` turns that lint (and its automatic fix, if it
  has one) off entirely; every key under `rules` can be omitted
  individually and falls back to `true`. The key names match the lints
  listed above: `trailing_whitespace`, `comma_spacing`,
  `forbidden_functions`, `slow_functions`, `unused_getter`, `unused_property`,
  `semicolon`, `float_int_conversion`, `strict_boolean`,
  `argument_types`, `numeric_comparison`, `indentation`,
  `cyclomatic_complexity`, `unreachable_statement`, `static_condition`, and
  `unused_local_variable`.

The app's formatting controls (trailing semicolons, indentation style,
indentation width) are backed by this file: on startup it reads the
config file for the most recently opened project and pre-selects those
controls accordingly, and any change made to them is written straight
back to the file, so the project's formatting settings persist between
sessions and can be shared/committed alongside the project. The
PapyrusCompile.exe path field on the Settings tab works the same way,
except it's pre-filled with an auto-detected path (see `compiler_path`
above) rather than a fixed default when the project has no explicit
override saved yet.

## Compiling a script

Each `.psc` file listed on the Lint results tab has a "Compile" button that
recompiles it with `PapyrusCompile.exe` (see `compiler_path` under
Configuration above for how that executable's path is resolved), so a fix
made in the code viewer can be tried out without leaving the app. It runs:

```
PapyrusCompile.exe "<source dir>" -f="<script name>.psc" -i="<source dir 1>;<source dir 2>" -o="<output dir>"
```

where `<source dir>` is the directory the `.psc` file lives in
(conventionally a `scripts/source` or `source/scripts` directory under the
project root) and `<output dir>` is its parent, matching the layout
Bethesda's tooling expects — a `Source` directory holding `.psc` files
inside the `Scripts` directory that receives the compiled `.pex` output.
`-i` is given both of those conventional source directories under the
project root, separated by `;` (PapyrusCompile.exe accepts multiple
import directories that way), so the script can still resolve imports
from the other layout even though it only lives in one of them.

The compiler's stdout/stderr is shown beneath the button once it finishes,
styled green on success and red on failure, so both a successful compile
and a reported error (a syntax error, a missing import, etc.) are visible
without checking a log file. If no compiler path is configured or
auto-detected, or the executable itself can't be run, that's reported the
same way rather than silently doing nothing.

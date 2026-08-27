# papyrus-lint
A linter for Bethesda's papyrus language to improve code quality.

## Implemented Lints

- **Trailing whitespace**: flag lines that end with trailing spaces or tabs.
- **Space after comma**: require whitespace after commas in argument lists.
- **Forbidden/discouraged function usage**: flag calls to functions listed
  in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls),
  with a configurable severity and an explanatory message per entry.
- **Getter usage without saving result**: flag standalone calls to functions
  whose names begin with `Get` (case-insensitively), because their return value
  is discarded.
- **Unused script properties**: flag `Property` declarations whose name is
  never referenced anywhere else in the script.
- **Semicolon at end of line**: require a trailing semicolon on each non-empty
  line or forbid terminal semicolons, according to the selected setting.
- **Implicit Float-to-Int conversion**: flag a Float value declared, assigned,
  returned, or passed as an argument into an Int-typed slot without an
  explicit `as Int` cast.
- **Strict boolean check**: flag `If`/`ElseIf`/`While` conditions that aren't
  already a `Bool` value or expression, instead of relying on Papyrus's
  implicit conversion to boolean. Only conditions whose type can be
  determined locally (locals, parameters, properties, literals, casts, and
  comparison/logical expressions) are checked; a condition that depends on
  a function call or a member access is left unflagged rather than risk a
  false positive.
- **Argument type check**: flag call-site arguments whose type doesn't match
  the callee's declared parameter type (e.g. passing a `String` where an
  `Int` is expected), allowing the implicit `Int`-to-`Float` widening
  Papyrus itself allows. Calls to functions declared in the same script are
  always checked; when linting a `.psc` file dropped in the app, calls to
  functions declared on other scripts under the project root (e.g.
  `SomeProperty.DoThing(...)`) are checked too, by resolving those scripts'
  signatures (including through `Extends`). A call whose target or argument
  type can't be determined is skipped rather than guessed at.
- **Strict numeric type check**: flag implicit comparisons (`==`, `!=`,
  `<`, `<=`, `>`, `>=`) between an `Int` value and a `Float` value without
  an explicit cast making the comparison exact. Only comparisons whose
  operand types can be determined locally are checked.
- **Formatting checks**: flag lines whose indentation doesn't match the
  configured style/width (`indentation`/`indentation_width`) for their
  nesting depth. A script whose structure can't be identified (e.g. it
  doesn't lex cleanly) is left unchecked rather than guessed at.
- **Slow function usage**: flag calls to functions listed in
  `rules/slow-functions.yaml` that have a faster equivalent available, and
  suggest the quicker alternative.
- **Cyclomatic complexity**: flag functions/events whose cyclomatic
  complexity (1 plus each `If`/`ElseIf` branch, `While` loop, and
  short-circuiting `&&`/`||` operator) exceeds a configurable threshold,
  as a `[warning]` above `cyclomatic_complexity_warning` (default 10) or
  an `[error]` above `cyclomatic_complexity_error` (default 20).

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed next to the
`.achlist` file you drop into the app. Any key it omits falls back to
its default:

```yaml
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
```

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
  `argument_types`, `numeric_comparison`, `indentation`, and
  `cyclomatic_complexity`.

The app's formatting controls (trailing semicolons, indentation style,
indentation width) are backed by this file: on startup it reads the
config file for the most recently opened project and pre-selects those
controls accordingly, and any change made to them is written straight
back to the file, so the project's formatting settings persist between
sessions and can be shared/committed alongside the project.

## Implemented Automatic Fixes

- **Trailing whitespace**: automatically strip trailing spaces or tabs from the end of lines.
- **Space after comma**: automatically insert a space after unspaced commas in argument lists.
- **Semicolon at end of line**: add required semicolons or remove terminal
  semicolons without discarding comment text.
- **Indentation**: automatically re-indent blocks using tabs or a configured
  number of spaces.

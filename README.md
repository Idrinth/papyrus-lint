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
- **Semicolon at end of line**: require a trailing semicolon on each non-empty
  line or forbid terminal semicolons, according to the selected setting.
- **Strict boolean check**: flag `If`/`ElseIf`/`While` conditions that aren't
  already a `Bool` value or expression, instead of relying on Papyrus's
  implicit conversion to boolean. Only conditions whose type can be
  determined locally (locals, parameters, properties, literals, casts, and
  comparison/logical expressions) are checked; a condition that depends on
  a function call or a member access is left unflagged rather than risk a
  false positive.

## Planned Lints

- **Strict numeric type check**: flag implicit conversions/comparisons between different numeric types (e.g. Int vs Float).
- **Slow function usage**: flag usage of functions that have a faster equivalent available, and suggest the quicker alternative.
- **Formatting checks**: enforce consistent indentation.

## Configuration

Lint/fix behavior is configured via an optional YAML file named
`papyrus-lint.yaml` (or `papyrus-lint.yml`), placed next to the
`.archlist` file you drop into the app. Any key it omits falls back to
its default:

```yaml
semicolon: false
indentation: tab
```

- `semicolon`: whether lines are required to end in a semicolon (`true`)
  or must not (`false`). Not currently read by the "Semicolon at end of
  line" lint/fix, which instead use the style selected in the app's UI.
- `indentation`: the expected indentation style, `tab` or `space`. Not
  currently read by the indentation automatic fix, which instead uses
  the style/width selected in the app's UI.

No lint currently reads this configuration — it's read and passed to
every check/fix job in preparation for a future lint that will use it.

## Implemented Automatic Fixes

- **Trailing whitespace**: automatically strip trailing spaces or tabs from the end of lines.
- **Space after comma**: automatically insert a space after unspaced commas in argument lists.
- **Semicolon at end of line**: add required semicolons or remove terminal
  semicolons without discarding comment text.
- **Indentation**: automatically re-indent blocks using tabs or a configured
  number of spaces.

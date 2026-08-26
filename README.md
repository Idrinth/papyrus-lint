# papyrus-lint
A linter for Bethesda's papyrus language to improve code quality.

## Implemented Lints

- **Trailing whitespace**: flag lines that end with trailing spaces or tabs.
- **Forbidden/discouraged function usage**: flag calls to functions listed
  in `rules/forbidden-functions.yaml` (e.g. slow or blocking native calls),
  with a configurable severity and an explanatory message per entry.
- **Getter usage without saving result**: flag standalone calls to functions
  whose names begin with `Get` (case-insensitively), because their return value
  is discarded.
- **Semicolon at end of line**: require a trailing semicolon on each non-empty
  line or forbid terminal semicolons, according to the selected setting.

## Planned Lints

- **Strict boolean check**: flag comparisons and conditions that rely on implicit boolean conversion instead of explicit boolean values.
- **Strict numeric type check**: flag implicit conversions/comparisons between different numeric types (e.g. Int vs Float).
- **Slow function usage**: flag usage of functions that have a faster equivalent available, and suggest the quicker alternative.
- **Formatting checks**: enforce consistent indentation and require a space after commas.

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
  or must not (`false`). Drives the planned "Semicolon at end of line"
  lint below.
- `indentation`: the expected indentation style, `tab` or `space`. Drives
  the planned indentation checks/fix below.

No lint currently reads this configuration — it's read and passed to
every check/fix job in preparation for the planned lints that will use
it.

## Implemented Automatic Fixes

- **Trailing whitespace**: automatically strip trailing spaces or tabs from the end of lines.
- **Semicolon at end of line**: add required semicolons or remove terminal
  semicolons without discarding comment text.

## Planned Automatic Fixes

- **Indentation**: automatically re-indent lines to match the configured indentation style.
- **Space after comma**: automatically insert a space after commas that lack one.

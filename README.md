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

## Planned Lints

- **Strict boolean check**: flag comparisons and conditions that rely on implicit boolean conversion instead of explicit boolean values.
- **Strict numeric type check**: flag implicit conversions/comparisons between different numeric types (e.g. Int vs Float).
- **Semicolon at end of line**: enforce a consistent, configurable rule of always requiring or always forbidding a trailing semicolon at the end of a line.
- **Slow function usage**: flag usage of functions that have a faster equivalent available, and suggest the quicker alternative.
- **Formatting checks**: enforce consistent indentation and require a space after commas.

## Planned Automatic Fixes

- **Indentation**: automatically re-indent lines to match the configured indentation style.
- **Space after comma**: automatically insert a space after commas that lack one.
- **Trailing whitespace**: automatically strip trailing spaces or tabs from the end of lines.

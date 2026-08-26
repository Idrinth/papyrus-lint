# papyrus-lint
A linter for Bethesda's papyrus language to improve code quality.

## Planned Lints

- **Strict boolean check**: flag comparisons and conditions that rely on implicit boolean conversion instead of explicit boolean values.
- **Strict numeric type check**: flag implicit conversions/comparisons between different numeric types (e.g. Int vs Float).
- **Semicolon at end of line**: enforce a consistent, configurable rule of always requiring or always forbidding a trailing semicolon at the end of a line.
- **Getter usage without saving result**: flag calls to getter functions whose return value is discarded instead of stored or used.
- **Slow function usage**: flag usage of functions that have a faster equivalent available, and suggest the quicker alternative.
- **Formatting checks**: enforce consistent indentation and require a space after commas.
- **Trailing whitespace**: flag lines that end with trailing spaces or tabs.

# Papyrus Lint for VS Code

A VS Code extension that surfaces [Papyrus Lint](../README.md) diagnostics
for `.psc` files directly in the editor, by shelling out to
`PapyrusLinterCLI --json`.

## Features

- Lints a `.psc` file automatically whenever it's opened or saved, by
  running `PapyrusLinterCLI --json <file>` and turning its
  [`JsonReport`](../app/crates/papyrus-lint-cli/src/lib.rs) into
  `vscode.Diagnostic`s (severity taken from each diagnostic's `level`;
  `rule` is shown as the diagnostic's code).
- **Papyrus Lint: Lint Current File** — re-lints on demand, from the
  command palette, the editor context menu, or a `.psc` file's explorer
  context menu.
- **Papyrus Lint: Fix Current File** — runs `PapyrusLinterCLI fix --json
  <file>`, which applies every automatic fix (see the project README) to
  the file on disk, then reports the diagnostics (if any) that remain.
  Unsaved changes are saved first, since the CLI only reads from disk.

Only the currently open/selected `.psc` file is linted or fixed — not the
whole project's `.achlist` — since that's the unit the CLI's `--json`
output is scoped to for a single-file invocation.

## Development

- `npm install`
- `npm run watch` (or `npm run compile` for a one-off build)
- Press F5 in VS Code (with this directory open) to launch an Extension
  Development Host with the extension loaded.

## Configuration

- `papyrusLint.cliPath`: path to the `PapyrusLinterCLI` executable
  (defaults to `PapyrusLinterCLI`, i.e. resolved from `PATH`).

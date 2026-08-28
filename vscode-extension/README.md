# Papyrus Lint for VS Code

A VS Code extension that surfaces [Papyrus Lint](../README.md) diagnostics
for `.psc` files directly in the editor, by shelling out to
`PapyrusLinterCLI`.

## Status

This is the initial project scaffold (extension manifest, TypeScript
build, and an `activate()`/`deactivate()` stub that registers an empty
diagnostic collection). It doesn't lint anything yet — invoking the CLI
and turning its output into `vscode.Diagnostic`s is still to be built.

## Development

- `npm install`
- `npm run watch` (or `npm run compile` for a one-off build)
- Press F5 in VS Code (with this directory open) to launch an Extension
  Development Host with the extension loaded.

## Configuration

- `papyrusLint.cliPath`: path to the `PapyrusLinterCLI` executable
  (defaults to `PapyrusLinterCLI`, i.e. resolved from `PATH`).

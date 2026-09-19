# Papyrus Lint for VS Code

A VS Code extension that surfaces [Papyrus Lint](../README.md) diagnostics
for `.psc` files directly in the editor.

## Features

- Lints a `.psc` file automatically whenever it's opened or saved. Each
  issue shows its severity and rule, with a link to the rule's documentation
  on the [project website](https://papyrus-lint.idrinth.de) when available.
- **Live linting**: as you type, the document's current (possibly unsaved)
  contents are checked after a short pause. This only updates diagnostics
  and never touches the file on disk. Live results skip cross-script checks
  and use the default configuration unless `papyrusLint.configPath` is set;
  saving runs the full project-aware lint. Controlled by the
  `papyrusLint.liveLint`/`papyrusLint.liveLintDebounceMs` settings below.
- **Schema validation for config files**: associates any
  `papyrus-lint.yml` / `papyrus-lint.yaml` (same `**/*.yml` + `**/*.yaml`
  pairing GitHub Actions uses for workflow files) with
  [`schema/papyrus-lint.schema.json`](../schema/papyrus-lint.schema.json),
  served at <https://papyrus-lint.idrinth.de/schema/papyrus-lint.schema.json>.
  The Red Hat YAML extension picks that `yamlValidation` contribution up
  for hover, completion, and diagnostics. The extension also activates
  when a YAML file is opened or when a workspace contains those filenames,
  not only on `onLanguage:papyrus`.
- **Papyrus Lint: Lint Current File** — re-lints on demand, from the
  command palette, the editor context menu, or a `.psc` file's explorer
  context menu.
- **Papyrus Lint: Fix Current File** — applies every available automatic
  fix (see the project README), then reports any diagnostics that remain.
  Unsaved changes are saved first, since the CLI only reads from disk.
- **Fix this issue** — a Quick Fix (lightbulb) offered on each individual
  diagnostic. It applies the fix for only that issue, leaving every other
  issue in the file untouched. Unsaved changes are saved first, same as
  fixing the whole file.
- **Papyrus Lint: Initialize Configuration** — creates a
  `papyrus-lint.yaml` (see the project
  [configuration reference](../docs/configuration.md)), without
  overwriting an existing one. Available from the command palette (prompts
  for the workspace folder to initialize when more than one is open) or a
  folder's explorer context menu (initializes that folder directly).
  Either way, prompts for a preset to start from — the built-in `strict`
  (the default), `standard`, or `careful`, or a custom preset name added
  via `PapyrusLinterCLI preset add` or the desktop app's "Save current
  settings as preset…" button.

Only the currently open or selected `.psc` file is linted or fixed, not the
whole project.

## Development

- `npm install`
- `npm run watch` (or `npm run compile` for a one-off build)
- Press F5 in VS Code (with this directory open) to launch an Extension
  Development Host with the extension loaded.

## Configuration

- `papyrusLint.cliPath`: optional path to a `PapyrusLinterCLI` executable
  or the desktop app's `PapyrusLinter` binary. When empty (the default),
  the extension downloads the platform-specific CLI from the GitHub
  release whose version matches the extension, caches it in VS Code's
  extension storage, and uses it automatically. Updating the extension
  downloads that new release's CLI (and drops the previously cached
  copy). Set this only to override the release CLI with a locally
  installed executable; its SHA-256 must match this release's baked CLI
  or GUI digest, and its `--version` output must match the extension's
  version.
- `papyrusLint.configPath`: path to a papyrus-lint config file to pass to
  the CLI via `--config`, overriding the `papyrus-lint.yaml`/`.yml` it
  would otherwise discover from the project root. Leave empty (the
  default) to use that discovery as normal.
- `papyrusLint.liveLint`: whether to lint a document's current, possibly
  unsaved contents as you type (see "Live linting" above). Defaults to
  `true`; set to `false` to only lint on open/save.
- `papyrusLint.liveLintDebounceMs`: how long, in milliseconds, to wait
  after the last keystroke before running a live lint. Defaults to `400`.

## Contact

- Discord: <https://discord.gg/idrinth>
- NexusMods: <https://www.nexusmods.com/skyrimspecialedition/mods/189862>
- GitHub: <https://github.com/idrinth/papyrus-lint>

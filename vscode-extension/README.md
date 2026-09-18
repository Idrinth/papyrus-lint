# Papyrus Lint for VS Code

A VS Code extension that surfaces [Papyrus Lint](../README.md) diagnostics
for `.psc` files directly in the editor, by shelling out to
`PapyrusLinterCLI --json`.

## Features

- Lints a `.psc` file automatically whenever it's opened or saved, by
  running `PapyrusLinterCLI --json <file>` and turning its
  [`JsonReport`](../app/crates/papyrus-lint-cli/src/lib.rs) into
  `vscode.Diagnostic`s (severity taken from each diagnostic's `level`;
  `rule` is shown as the diagnostic's code, clickable straight to that
  rule's own documentation on the
  [project website](https://papyrus-lint.idrinth.de) when its `doc_url`
  is known — a compiler-reported diagnostic keeps a plain, unlinked code).
- **Live linting**: as you type, the document's current (possibly unsaved)
  contents are also linted directly via `PapyrusLinterCLI --json --blob
  <text>`, debounced so a burst of keystrokes triggers one lint pass
  shortly after the last of them rather than one per keystroke. This runs
  alongside the open/save lint above and only updates diagnostics — it
  never touches the file on disk. Since `--blob` lints in isolation (no
  project root to resolve), it skips cross-script checks and, unless
  `papyrusLint.configPath` is set, the project's own
  `papyrus-lint.yaml`/`.yml` in favor of the CLI's defaults; the full,
  project-aware lint still runs again on save. Controlled by the
  `papyrusLint.liveLint`/`papyrusLint.liveLintDebounceMs` settings below.
- **Papyrus Lint: Lint Current File** — re-lints on demand, from the
  command palette, the editor context menu, or a `.psc` file's explorer
  context menu.
- **Papyrus Lint: Fix Current File** — runs `PapyrusLinterCLI fix --json
  <file>`, which applies every automatic fix (see the project README) to
  the file on disk, then reports the diagnostics (if any) that remain.
  Unsaved changes are saved first, since the CLI only reads from disk.
- **Fix this issue** — a Quick Fix (lightbulb) offered on each individual
  diagnostic, which runs `PapyrusLinterCLI fix --type <rule> --line <line>
  --json <file>` to apply just that diagnostic's own rule on its own line,
  leaving every other issue in the file untouched. Unsaved changes are
  saved first, same as fixing the whole file.
- **Papyrus Lint: Initialize Configuration** — runs `PapyrusLinterCLI init
  [--preset <name>]` to scaffold a `papyrus-lint.yaml` (see the project
  [configuration reference](../docs/configuration.md)), without
  overwriting an existing one. Available from the command palette (prompts
  for the workspace folder to initialize when more than one is open) or a
  folder's explorer context menu (initializes that folder directly).
  Either way, prompts for a preset to start from — the built-in `strict`
  (the default), `standard`, or `careful`, or a custom preset name added
  via `PapyrusLinterCLI preset add` or the desktop app's "Save current
  settings as preset…" button.

Only the currently open/selected `.psc` file is linted or fixed — not the
whole project's `.achlist` — since that's the unit the CLI's `--json`
output is scoped to for a single-file invocation.

## Development

- `npm install`
- `npm run watch` (or `npm run compile` for a one-off build)
- Press F5 in VS Code (with this directory open) to launch an Extension
  Development Host with the extension loaded.

## Configuration

- `papyrusLint.cliPath`: optional path to a `PapyrusLinterCLI` executable.
  When empty (the default), the extension downloads the platform-specific CLI
  from the GitHub release whose version matches the extension, caches it in
  VS Code's extension storage, and uses it automatically. Updating the
  extension downloads that new release's CLI (and drops the previously cached
  copy). Set this only to override the release CLI with a locally installed
  executable.
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

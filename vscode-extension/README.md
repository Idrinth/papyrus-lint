# Papyrus Lint for VS Code

A VS Code extension that surfaces [Papyrus Lint](../README.md) diagnostics
for `.psc` files directly in the editor. It is only handling Skyrim SE/AE
at the moment, but might already work on Fallout 4 or Starfield partially.
Full support for the later two games is in the works.

Attention: If you don't provide an executable for this extension to lint with,
it will download the matching github release executable for your system. This
executable and any other provided are validated by hash to make sure that you
are not executing something untrusted.

## Features

- Lints a `.psc` file automatically whenever it's opened or saved. Each
  issue shows its severity and rule, with a link to the rule's documentation
  on the [project website](https://papyrus-lint.idrinth.de) when available.
- **Live linting**: as you type, the document's current (possibly unsaved)
  contents are checked after a short pause. This only updates diagnostics
  and never touches the file on disk. Live results skip cross-script checks,
  but still apply the workspace's own `papyrus-lint.yaml`/`.yml` (see
  `papyrusLint.configPath` below — the extension finds it itself via VS
  Code's workspace-folder API rather than relying on the CLI's on-disk
  project-root discovery, since a live, unsaved lint has no real file
  position for that to walk up from); saving runs the full project-aware
  lint. Controlled by the `papyrusLint.liveLint`/`papyrusLint.liveLintDebounceMs`
  settings below.
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
- **Ignore this lint for the line** — a Quick Fix that adds (or extends) a
  `; @disable <rule>` comment on the diagnostic's own line, silencing that
  rule there only. The buffer is left unsaved so the comment can
  be undone; diagnostics refresh from the in-memory text.
- **Ignore this lint for the file** — a Quick Fix that adds (or extends) a
  `; @disable-file <rule>` comment in the current script, silencing that
  rule for the whole file. The buffer is left unsaved so the comment can
  be undone; diagnostics refresh from the in-memory text.
- **Ignore this lint for the project** — a Quick Fix that turns the rule
  off in the workspace's `papyrus-lint.yaml`/`.yml` (`rules.<id>: false`).
  If that file already exists it is edited in place (comments and other
  keys are kept); if the workspace has none yet, a minimal
  `papyrus-lint.yaml` is created so you don't have to run Initialize
  Configuration first. Compiler-reported diagnostics (`compiler-error`)
  have no such toggle and don't get these ignore actions.
- **Add ; @nodiscard flag** — a lightbulb action offered on a function
  header that returns a value or is `Native` and isn't flagged already. It
  adds (or extends) a trailing `; @nodiscard` comment marking the function
  for the `unused-nodiscard` rule's discarded-result check, applied
  directly to the buffer with no CLI round-trip needed.
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
  version. Applies to the whole VS Code window.
- `papyrusLint.configPath`: path to a papyrus-lint config file to pass to
  the CLI via `--config`, overriding auto-detection. Leave empty (the
  default) to auto-detect instead: the extension itself looks for a
  `papyrus-lint.yaml`/`.yml` by walking up from the linted file to its
  enclosing VS Code workspace folder (found via `workspace.getWorkspaceFolder`),
  so a config placed at the workspace root is picked up for live linting
  too, not only a saved file's own CLI-side project-root discovery.
- `papyrusLint.liveLint`: whether to lint a document's current, possibly
  unsaved contents as you type (see "Live linting" above). Defaults to
  `true`; set to `false` to only lint on open/save.
- `papyrusLint.liveLintDebounceMs`: how long, in milliseconds, to wait
  after the last keystroke before running a live lint. Defaults to `400`.

`configPath`, `liveLint`, and `liveLintDebounceMs` are all
[resource-scoped](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration)
settings, so in a multi-root workspace each folder can set its own value
(e.g. a `.vscode/settings.json` in one folder pointing at that folder's
own `papyrus-lint.yaml`, or turning live linting off for a folder with a
much larger set of scripts) instead of one value applying to every folder.

## Contact

<!--CONTACT-LINKS-->

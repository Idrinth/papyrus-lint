import * as vscode from 'vscode';

function configuredCliPath(): string | undefined {
  const configured = vscode.workspace.getConfiguration('papyrusLint').get<string>('cliPath', '');
  return configured.trim() || undefined;
}

/** The `papyrusLint.configPath` setting, or `undefined` if unset/blank, in which case
 * the CLI falls back to its own papyrus-lint.yaml/.yml discovery from the project root. */
export function configPath(): string | undefined {
  const configured = vscode.workspace.getConfiguration('papyrusLint').get<string>('configPath', '');
  return configured.trim() === '' ? undefined : configured;
}

/** Prepends `--config <path>` to `args` when `papyrusLint.configPath` is set. */
export function withConfigOverride(args: string[]): string[] {
  const override = configPath();
  return override ? ['--config', override, ...args] : args;
}

/** Whether live, as-you-type linting (via `--blob`, see `PapyrusLinter.lintBlob`)
 * is enabled. Defaults to on; a user can turn it off if spawning the CLI on every
 * pause in typing is more overhead than they want. */
export function liveLintEnabled(): boolean {
  return vscode.workspace.getConfiguration('papyrusLint').get<boolean>('liveLint', true);
}

/** How long to wait, in milliseconds, after the last keystroke in a Papyrus
 * document before running a live `--blob` lint of its current (possibly unsaved)
 * contents. Exposed as a setting mainly so tests can drive it down to `0`. */
export function liveLintDebounceMs(): number {
  return vscode.workspace.getConfiguration('papyrusLint').get<number>('liveLintDebounceMs', 400);
}

export function resolveCliPath(): string | undefined {
  return configuredCliPath();
}

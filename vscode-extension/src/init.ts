import * as vscode from 'vscode';
import { runCli, showCliLaunchFailure } from './cli';

const CUSTOM_INIT_PRESET_LABEL = 'Custom preset name…';

interface InitPresetQuickPickItem extends vscode.QuickPickItem {
  /** The `--preset` value to pass, or `''` to omit `--preset` entirely (the CLI's own
   * "strict" default), for every item except the custom one below. */
  preset?: string;
}

interface InitGameQuickPickItem extends vscode.QuickPickItem {
  /** The `--game` value. Starfield parses but is not supported by the linter yet. */
  game: 'skyrim' | 'fallout4';
}

function initPresetQuickPickItems(): InitPresetQuickPickItem[] {
  return [
    { label: 'strict (default)', preset: '', description: 'Every lint rule enabled at its strictest' },
    { label: 'standard', preset: 'standard', description: 'A more relaxed baseline' },
    { label: 'careful', preset: 'careful', description: 'The most relaxed baseline' },
    { label: CUSTOM_INIT_PRESET_LABEL, description: 'A preset added via "preset add" or the desktop app' },
  ];
}

function initGameQuickPickItems(): InitGameQuickPickItem[] {
  return [
    { label: 'Skyrim (default)', game: 'skyrim', description: 'Skyrim Special Edition / Anniversary Edition' },
    { label: 'Fallout 4', game: 'fallout4', description: 'Fallout 4' },
  ];
}

/** Prompts for the `--preset` value `papyrusLint.initializeConfig` should pass to
 * `PapyrusLinterCLI init`: `''` to omit the flag (the CLI's own "strict" default), a
 * built-in preset name, or a custom one typed into a follow-up input box. Returns
 * `undefined` if the user cancels at either step. */
async function pickInitPreset(): Promise<string | undefined> {
  const picked = await vscode.window.showQuickPick(initPresetQuickPickItems(), {
    placeHolder: 'Select a papyrus-lint.yaml preset to start from',
  });
  if (!picked) {
    return undefined;
  }
  if (picked.label !== CUSTOM_INIT_PRESET_LABEL) {
    return picked.preset ?? '';
  }
  const custom = await vscode.window.showInputBox({
    prompt: 'Name of a custom preset (added via "PapyrusLinterCLI preset add", or saved from the desktop app)',
    placeHolder: 'e.g. team-style',
  });
  return custom?.trim() || undefined;
}

/** Prompts for the `--game` value `papyrusLint.initializeConfig` should pass.
 * `init` requires `--game`. Returns `undefined` if the user cancels. */
async function pickInitGame(): Promise<string | undefined> {
  const picked = await vscode.window.showQuickPick(initGameQuickPickItems(), {
    placeHolder: 'Select the game this project targets',
  });
  return picked?.game;
}

/** Resolves the project directory `papyrusLint.initializeConfig` should run `init` in:
 * the workspace's sole folder, a prompt when several are open, or `undefined` (after
 * showing an error) when no folder is open at all. */
async function resolveInitDirectory(): Promise<string | undefined> {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    void vscode.window.showErrorMessage('Papyrus Lint: open a folder or workspace first.');
    return undefined;
  }
  if (folders.length === 1) {
    return folders[0].uri.fsPath;
  }
  const picked = await vscode.window.showWorkspaceFolderPick({
    placeHolder: 'Select the project folder to initialize a papyrus-lint.yaml in',
  });
  return picked?.uri.fsPath;
}

/** Runs `PapyrusLinterCLI init --game <name> [--preset <name>]` in `directory`, e.g. a
 * right-clicked explorer folder or a workspace folder resolved via
 * `resolveInitDirectory`, prompting for a preset and a target game first. */
export async function initializeConfig(output: vscode.OutputChannel, uri?: vscode.Uri): Promise<void> {
  const directory = uri ? uri.fsPath : await resolveInitDirectory();
  if (!directory) {
    return;
  }
  const preset = await pickInitPreset();
  if (preset === undefined) {
    return;
  }
  const game = await pickInitGame();
  if (!game) {
    return;
  }

  const args = ['init', '--game', game];
  if (preset) {
    args.push('--preset', preset);
  }
  const result = await runCli(args, directory);
  if (result.code === -1) {
    showCliLaunchFailure(result);
    return;
  }
  if (result.code !== 0) {
    const message = result.stderr.trim() || 'failed to initialize config.';
    output.appendLine(`papyrus-lint: ${message}`);
    void vscode.window.showErrorMessage(`Papyrus Lint: ${message}`);
    return;
  }
  void vscode.window.showInformationMessage(`Papyrus Lint: ${result.stdout.trim()}`);
}

import * as vscode from 'vscode';
import { runCli, showCliLaunchFailure } from './cli';

const CUSTOM_INIT_PRESET_LABEL = 'Custom preset name…';

interface InitPresetQuickPickItem extends vscode.QuickPickItem {
  /** The `--preset` value to pass, or `''` to omit `--preset` entirely (the CLI's own
   * "strict" default), for every item except the custom one below. */
  preset?: string;
}

interface InitGameQuickPickItem extends vscode.QuickPickItem {
  /** The `--game` value. */
  game: 'skyrim' | 'fallout4' | 'starfield';
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
    { label: 'Starfield', game: 'starfield', description: 'Starfield' },
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

/** The VS Code workspace folder that should own `papyrus-lint.yaml` for `uri`:
 * the folder that contains it, or one whose own path is `uri` (a right-clicked
 * workspace root). `getWorkspaceFolder` already treats the folder root as
 * inside itself, so a workspace with no subfolders still resolves here instead
 * of falling through to `uri.fsPath` (which for a file, or for a resource VS
 * Code reports as the parent of an empty folder, would write the config
 * outside the workspace). */
function workspaceFolderFor(uri: vscode.Uri): vscode.WorkspaceFolder | undefined {
  return vscode.workspace.getWorkspaceFolder(uri)
    ?? vscode.workspace.workspaceFolders?.find((folder) => folder.uri.fsPath === uri.fsPath);
}

/** Resolves the project directory `papyrusLint.initializeConfig` should run `init` in:
 * the workspace folder that contains `uri` when one is given (a right-clicked
 * file, nested folder, or the workspace root itself), the workspace's sole
 * folder, a prompt when several are open, or `undefined` (after showing an
 * error) when no folder is open at all.
 *
 * Always the workspace folder root — never a nested explorer path, and never a
 * path outside the workspace. A flat workspace (scripts sitting next to the
 * folder root, no subfolders) and a deeply nested one (`Data/Scripts/Source/…`)
 * therefore both initialize `papyrus-lint.yaml` at the same place the rest of
 * the extension already looks for it (`configPath` / `configPathForWrite`). */
async function resolveInitDirectory(uri?: vscode.Uri): Promise<string | undefined> {
  if (uri) {
    const folder = workspaceFolderFor(uri);
    if (folder) {
      return folder.uri.fsPath;
    }
  }
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

/** Runs `PapyrusLinterCLI init --game <name> [--preset <name>]` in the resolved
 * workspace folder (see `resolveInitDirectory`), prompting for a preset and a
 * target game first. */
export async function initializeConfig(output: vscode.OutputChannel, uri?: vscode.Uri): Promise<void> {
  const directory = await resolveInitDirectory(uri);
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

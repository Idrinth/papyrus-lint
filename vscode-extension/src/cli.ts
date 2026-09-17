import { execFile } from 'child_process';
import * as vscode from 'vscode';
import { ensureReleaseCli } from './cliDownload';
import { resolveCliPath } from './config';

export interface CliResult {
  /** The process exit code, or `-1` if the CLI executable itself couldn't be launched. */
  code: number;
  stdout: string;
  stderr: string;
}

let automaticCli: Promise<string> | undefined;
let automaticCliStorage = '';
let extensionVersion = '';

/** Records this activation's storage/version and drops any in-flight automatic
 * download so a later failure can be retried. */
export function configureCli(storageDirectory: string, version: string): void {
  automaticCliStorage = storageDirectory;
  extensionVersion = version;
  automaticCli = undefined;
}

export function showCliLaunchFailure(result: CliResult): void {
  void vscode.window.showErrorMessage(
    `Papyrus Lint: could not download or run its CLI. Set "papyrusLint.cliPath" to override it. ` +
      `(${result.stderr.trim()})`,
  );
}

export async function runCli(args: string[], cwd: string): Promise<CliResult> {
  let executable: string;
  try {
    const configured = resolveCliPath();
    if (configured) {
      executable = configured;
    } else {
      automaticCli ??= ensureReleaseCli(automaticCliStorage, extensionVersion);
      executable = await automaticCli;
    }
  } catch (error) {
    automaticCli = undefined;
    return { code: -1, stdout: '', stderr: error instanceof Error ? error.message : String(error) };
  }
  return new Promise((resolve) => {
    execFile(executable, args, { cwd, maxBuffer: 10 * 1024 * 1024 }, (error, stdout, stderr) => {
      if (error && typeof (error as NodeJS.ErrnoException).code !== 'number') {
        // The executable itself couldn't be launched (e.g. not found on PATH).
        resolve({ code: -1, stdout, stderr: error.message });
        return;
      }
      const code = error ? ((error as NodeJS.ErrnoException).code as unknown as number) : 0;
      resolve({ code, stdout, stderr });
    });
  });
}

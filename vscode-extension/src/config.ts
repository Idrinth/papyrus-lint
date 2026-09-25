import { promises as fs } from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

function configuredCliPath(): string | undefined {
  const configured = vscode.workspace.getConfiguration('papyrusLint').get<string>('cliPath', '');
  return configured.trim() || undefined;
}

/** The config file names the CLI itself recognizes, checked in the same order (see
 * `papyrus_lint_config::project_file::CONFIG_FILE_NAMES`). */
const CONFIG_FILE_NAMES = ['papyrus-lint.yaml', 'papyrus-lint.yml'];

/** The `papyrusLint.configPath` setting for `documentUri`, or `undefined` if
 * unset/blank. `configPath` is a `resource`-scoped setting (see `package.json`), so
 * each workspace folder in a multi-root workspace can point at its own config file;
 * `documentUri` is passed as the resource so VS Code resolves the value that applies
 * to the folder that actually contains it, rather than a single workspace-wide value. */
function configuredConfigPath(documentUri: vscode.Uri): string | undefined {
  const configured = vscode.workspace.getConfiguration('papyrusLint', documentUri).get<string>('configPath', '');
  return configured.trim() === '' ? undefined : configured;
}

/** Walks up from `startDir` to (and including) `workspaceRoot`, returning the path to
 * the first `papyrus-lint.yaml`/`.yml` found, or `undefined` if neither exists
 * anywhere in between. Mirrors the CLI's own config-file discovery
 * (`papyrus_lint_core::project_root::find_config_file_root`), but bounded to the
 * current VS Code workspace folder instead of walking all the way to the filesystem
 * root, since that's the boundary `getWorkspaceFolder` below already gives us. */
async function findConfigInWorkspace(startDir: string, workspaceRoot: string): Promise<string | undefined> {
  const relativeStart = path.relative(workspaceRoot, startDir);
  if (relativeStart.startsWith('..') || path.isAbsolute(relativeStart)) {
    // startDir isn't actually under workspaceRoot (shouldn't happen, since
    // getWorkspaceFolder already established that it is); nothing to walk.
    return undefined;
  }

  let dir = startDir;
  for (;;) {
    for (const name of CONFIG_FILE_NAMES) {
      const candidate = path.join(dir, name);
      try {
        if ((await fs.stat(candidate)).isFile()) {
          return candidate;
        }
      } catch {
        // Not found (or not readable) here; keep looking further up.
      }
    }
    if (path.relative(workspaceRoot, dir) === '') {
      return undefined;
    }
    dir = path.dirname(dir);
  }
}

/** Resolves the `papyrus-lint.yaml`/`.yml` that should be used for `documentUri`: the
 * `papyrusLint.configPath` setting when set, otherwise a config file found by walking
 * up from the document towards its enclosing VS Code workspace folder — found via
 * `vscode.workspace.getWorkspaceFolder`, the same workspace-roots API
 * `papyrusLint.initializeConfig` already uses to pick a project directory (see
 * `resolveInitDirectory` in `init.ts`). Detecting it here, rather than leaving it
 * entirely to the CLI's own project-root discovery, is what lets live, as-you-type
 * `--blob` linting — which otherwise skips that discovery altogether, see
 * `PapyrusLinter.lintBlob` below — pick up a config placed at the workspace root too,
 * and lets on-save linting find it without depending on whichever CLI release happens
 * to already be installed. Returns `undefined` when neither finds one, leaving a
 * saved file's own CLI invocation to fall back to the CLI's own discovery as before. */
export async function configPath(documentUri: vscode.Uri): Promise<string | undefined> {
  const configured = configuredConfigPath(documentUri);
  if (configured) {
    return configured;
  }
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(documentUri);
  if (!workspaceFolder) {
    return undefined;
  }
  return findConfigInWorkspace(path.dirname(documentUri.fsPath), workspaceFolder.uri.fsPath);
}

/** Resolves the config file an "ignore this lint for the project" edit should
 * write: the same path `configPath` would pass as `--config` when one exists
 * (including `papyrusLint.configPath`), otherwise `papyrus-lint.yaml` in the
 * document's VS Code workspace folder so the ignore can still land without
 * requiring `initializeConfig` first. Returns `undefined` when there's no
 * workspace folder to write into and no override path either. */
export async function configPathForWrite(documentUri: vscode.Uri): Promise<string | undefined> {
  const existing = await configPath(documentUri);
  if (existing) {
    return existing;
  }
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(documentUri);
  if (!workspaceFolder) {
    return undefined;
  }
  return path.join(workspaceFolder.uri.fsPath, CONFIG_FILE_NAMES[0]);
}

/** Inserts `--config <path>` after the CLI subcommand in `args` when a config is
 * resolved for `documentUri` (see `configPath` above). `--config` is a flag of
 * `lint`/`fix`/`doctor`, not a global flag: putting it before the subcommand is a
 * usage error on PapyrusLinterCLI 2.x (exit status 2, USAGE on stderr). */
export async function withConfigOverride(args: string[], documentUri: vscode.Uri): Promise<string[]> {
  const override = await configPath(documentUri);
  if (!override) {
    return args;
  }
  const [subcommand, ...rest] = args;
  return [subcommand, '--config', override, ...rest];
}

/** Whether live, as-you-type linting (via `--blob`, see `PapyrusLinter.lintBlob`)
 * is enabled for `documentUri`. Defaults to on; a user can turn it off if spawning
 * the CLI on every pause in typing is more overhead than they want, either for the
 * whole window or, since this is a `resource`-scoped setting, for just one workspace
 * folder in a multi-root workspace (e.g. one holding a much larger mod). */
export function liveLintEnabled(documentUri: vscode.Uri): boolean {
  return vscode.workspace.getConfiguration('papyrusLint', documentUri).get<boolean>('liveLint', true);
}

/** How long to wait, in milliseconds, after the last keystroke in `documentUri`
 * before running a live `--blob` lint of its current (possibly unsaved) contents.
 * `resource`-scoped, like `liveLintEnabled` above, so it can also be tuned per
 * workspace folder; exposed as a setting mainly so tests can drive it down to `0`. */
export function liveLintDebounceMs(documentUri: vscode.Uri): number {
  return vscode.workspace.getConfiguration('papyrusLint', documentUri).get<number>('liveLintDebounceMs', 400);
}

export function resolveCliPath(): string | undefined {
  return configuredCliPath();
}

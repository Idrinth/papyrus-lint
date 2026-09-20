import Module from 'node:module';
import path from 'node:path';

const extensionModule = path.resolve('out-test/src/extension.js');
const compiledSrc = `${path.sep}out-test${path.sep}src${path.sep}`;
const originalLoad = Module._load;

export function evictCompiledModules() {
  for (const key of Object.keys(Module._cache)) {
    if (key.includes(compiledSrc)) {
      delete Module._cache[key];
    }
  }
}

export function restoreModules() {
  Module._load = originalLoad;
  evictCompiledModules();
}

export function createHarness({
  cliPath = '/tools/PapyrusLinterCLI',
  configPath = '',
  liveLint = true,
  liveLintDebounceMs = 0,
  textDocuments = [],
  releaseCli = async () => '/downloaded/PapyrusLinterCLI',
  result,
  versionResult,
  workspaceFolders,
  quickPickResult,
  inputBoxResult,
  workspaceFolderPickResult,
  /** Per-workspace-folder overrides for the `resource`-scoped settings
   * (`configPath`, `liveLint`, `liveLintDebounceMs`), keyed by that folder's
   * `uri.fsPath`. Lets multi-root tests give each folder its own value, the
   * way VS Code resolves a resource-scoped setting against the folder that
   * contains the resource passed to `getConfiguration`. */
  folderConfig = {},
  applyEditResult = true,
} = {}) {
  const commands = new Map();
  const listeners = {};
  const messages = { error: [], information: [], warning: [] };
  const diagnostics = {
    deleted: [],
    published: [],
    delete(uri) { this.deleted.push(uri); },
    set(uri, entries) { this.published.push([uri, entries]); },
  };
  const output = { lines: [], appendLine(line) { this.lines.push(line); }, dispose() {} };
  const codeActionProviders = [];
  const appliedEdits = [];
  const vscode = {
    CodeAction: class {
      constructor(title, kind) {
        Object.assign(this, { title, kind });
      }
    },
    CodeActionKind: { QuickFix: 'quickfix', RefactorRewrite: 'refactor.rewrite' },
    Diagnostic: class {
      constructor(range, message, severity) {
        Object.assign(this, { range, message, severity });
      }
    },
    DiagnosticSeverity: { Error: 0, Warning: 1, Information: 2 },
    Position: class {
      constructor(line, character) {
        Object.assign(this, { line, character });
      }
    },
    Range: class {
      constructor(startLine, startColumn, endLine, endColumn) {
        Object.assign(this, { startLine, startColumn, endLine, endColumn });
      }
    },
    WorkspaceEdit: class {
      constructor() {
        this.inserts = [];
        this.replacements = [];
      }
      insert(uri, position, newText) {
        this.inserts.push({ uri, position, newText });
      }
      replace(uri, range, newText) {
        this.replacements.push({ uri, range, newText });
      }
    },
    Uri: {
      parse: (value) => ({ scheme: value.split(':')[0], toString: () => value }),
      file: (fsPath) => uri(fsPath),
    },
    commands: {
      registerCommand(name, callback) {
        commands.set(name, callback);
        return { dispose() {} };
      },
    },
    languages: {
      createDiagnosticCollection: () => diagnostics,
      registerCodeActionsProvider(selector, provider) {
        codeActionProviders.push({ selector, provider });
        return { dispose() {} };
      },
    },
    window: {
      activeTextEditor: undefined,
      createOutputChannel: () => output,
      showErrorMessage: (message) => messages.error.push(message),
      showInformationMessage: (message) => messages.information.push(message),
      showWarningMessage: (message) => messages.warning.push(message),
      showQuickPick: async () => quickPickResult,
      showInputBox: async () => inputBoxResult,
      showWorkspaceFolderPick: async () => workspaceFolderPickResult,
    },
    workspace: {
      getConfiguration: (_section, resource) => ({
        get: (key, fallback) => {
          const overrides = resource ? findFolderOverrides(resource) : undefined;
          const value = overrides?.[key] ?? { cliPath, configPath, liveLint, liveLintDebounceMs }[key];
          return value ?? fallback;
        },
      }),
      getWorkspaceFolder,
      onDidChangeTextDocument: (callback) => registerListener('change', callback),
      onDidCloseTextDocument: (callback) => registerListener('close', callback),
      onDidOpenTextDocument: (callback) => registerListener('open', callback),
      onDidSaveTextDocument: (callback) => registerListener('save', callback),
      textDocuments,
      workspaceFolders,
      applyEdit: async (edit) => {
        appliedEdits.push(edit);
        if (!applyEditResult) {
          return false;
        }
        for (const replacement of edit.replacements ?? []) {
          const document = textDocuments.find((candidate) => (
            candidate.uri === replacement.uri
            || candidate.uri?.toString?.() === replacement.uri?.toString?.()
            || candidate.uri?.fsPath === replacement.uri?.fsPath
          ));
          if (document) {
            document.getText = () => replacement.newText;
            document.isDirty = true;
          }
        }
        return true;
      },
      openTextDocument: async (openUri) => {
        const found = textDocuments.find((candidate) => (
          candidate.uri === openUri
          || candidate.uri?.toString?.() === openUri?.toString?.()
          || candidate.uri?.fsPath === openUri?.fsPath
        ));
        if (found) {
          return found;
        }
        throw new Error(`File not found: ${openUri?.fsPath ?? openUri}`);
      },
    },
  };
  const execCalls = [];

  function getWorkspaceFolder(docUri) {
    return (workspaceFolders ?? []).find((folder) => {
      const base = folder.uri.fsPath.endsWith('/') ? folder.uri.fsPath : `${folder.uri.fsPath}/`;
      return docUri.fsPath === folder.uri.fsPath || docUri.fsPath.startsWith(base);
    });
  }

  function findFolderOverrides(resource) {
    const folder = getWorkspaceFolder(resource);
    return folder ? folderConfig[folder.uri.fsPath] : undefined;
  }

  function registerListener(name, callback) {
    listeners[name] = callback;
    return { dispose() {} };
  }

  Module._load = function (request, parent, isMain) {
    if (request === 'vscode') return vscode;
    if (request === './cliDownload' || request.endsWith('/cliDownload')) {
      return {
        ensureReleaseCli: releaseCli,
        verifyConfiguredExecutable: async () => undefined,
      };
    }
    if (request === 'child_process') {
      return {
        execFile(executable, args, options, callback) {
          if (args.length === 1 && args[0] === '--version') {
            const response = versionResult ?? {
              error: null,
              stdout: 'PapyrusLinterCLI 1.2.3\n',
              stderr: '',
            };
            callback(response.error, response.stdout, response.stderr);
            return;
          }
          execCalls.push({ executable, args, options });
          if (typeof result === 'function') {
            result({ executable, args, options, callback });
            return;
          }
          const response = result ?? { error: null, stdout: validReport(), stderr: '' };
          callback(response.error, response.stdout, response.stderr);
        },
      };
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  evictCompiledModules();
  const extension = Module._load(extensionModule, null, false);
  const context = {
    extension: { packageJSON: { version: '1.2.3' } },
    globalStorageUri: { fsPath: '/extension-storage' },
    subscriptions: [],
  };
  extension.activate(context);
  return {
    appliedEdits,
    codeActionProviders,
    commands,
    context,
    diagnostics,
    execCalls,
    extension,
    listeners,
    messages,
    output,
    vscode,
  };
}

export function uri(fsPath, scheme = 'file') {
  return { fsPath, scheme, toString: () => `${scheme}:${fsPath}` };
}

export function validReport(overrides = {}) {
  return JSON.stringify({
    files: [{ path: '/project/Test.psc', diagnostics: [] }],
    scripts_checked: 1,
    files_with_diagnostics: 0,
    total_diagnostics: 0,
    files_fixed: null,
    success: true,
    ...overrides,
  });
}

export function papyrusDocument(fsPath, text) {
  const lines = text.split(/\r?\n/);
  return {
    uri: uri(fsPath),
    languageId: 'papyrus',
    isDirty: true,
    getText: () => text,
    lineAt: (line) => ({
      text: lines[line],
      range: { end: { line, character: lines[line].length } },
    }),
    positionAt: (offset) => ({ line: 0, character: offset }),
    async save() { return true; },
  };
}

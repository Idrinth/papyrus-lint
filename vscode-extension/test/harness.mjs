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
  workspaceFolders,
  quickPickResult,
  inputBoxResult,
  workspaceFolderPickResult,
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
  const vscode = {
    CodeAction: class {
      constructor(title, kind) {
        Object.assign(this, { title, kind });
      }
    },
    CodeActionKind: { QuickFix: 'quickfix' },
    Diagnostic: class {
      constructor(range, message, severity) {
        Object.assign(this, { range, message, severity });
      }
    },
    DiagnosticSeverity: { Error: 0, Warning: 1, Information: 2 },
    Range: class {
      constructor(startLine, startColumn, endLine, endColumn) {
        Object.assign(this, { startLine, startColumn, endLine, endColumn });
      }
    },
    Uri: {
      parse: (value) => ({ scheme: value.split(':')[0], toString: () => value }),
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
      getConfiguration: () => ({
        get: (key, fallback) => ({ cliPath, configPath, liveLint, liveLintDebounceMs })[key] ?? fallback,
      }),
      onDidChangeTextDocument: (callback) => registerListener('change', callback),
      onDidCloseTextDocument: (callback) => registerListener('close', callback),
      onDidOpenTextDocument: (callback) => registerListener('open', callback),
      onDidSaveTextDocument: (callback) => registerListener('save', callback),
      textDocuments,
      workspaceFolders,
    },
  };
  const execCalls = [];

  function registerListener(name, callback) {
    listeners[name] = callback;
    return { dispose() {} };
  }

  Module._load = function (request, parent, isMain) {
    if (request === 'vscode') return vscode;
    if (request === './cliDownload' || request.endsWith('/cliDownload')) {
      return { ensureReleaseCli: releaseCli };
    }
    if (request === 'child_process') {
      return {
        execFile(executable, args, options, callback) {
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
  return { uri: uri(fsPath), languageId: 'papyrus', isDirty: true, getText: () => text };
}

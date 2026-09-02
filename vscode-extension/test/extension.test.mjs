import assert from 'node:assert/strict';
import Module from 'node:module';
import path from 'node:path';
import { afterEach, describe, it } from 'node:test';

const extensionModule = path.resolve('out-test/src/extension.js');
const originalLoad = Module._load;

function createHarness({
  cliPath = '/tools/PapyrusLinterCLI',
  configPath = '',
  textDocuments = [],
  releaseCli = async () => '/downloaded/PapyrusLinterCLI',
  result,
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
  const vscode = {
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
    commands: {
      registerCommand(name, callback) {
        commands.set(name, callback);
        return { dispose() {} };
      },
    },
    languages: { createDiagnosticCollection: () => diagnostics },
    window: {
      activeTextEditor: undefined,
      createOutputChannel: () => output,
      showErrorMessage: (message) => messages.error.push(message),
      showInformationMessage: (message) => messages.information.push(message),
      showWarningMessage: (message) => messages.warning.push(message),
    },
    workspace: {
      getConfiguration: () => ({
        get: (key, fallback) => ({ cliPath, configPath })[key] ?? fallback,
      }),
      onDidCloseTextDocument: (callback) => registerListener('close', callback),
      onDidOpenTextDocument: (callback) => registerListener('open', callback),
      onDidSaveTextDocument: (callback) => registerListener('save', callback),
      textDocuments,
    },
  };
  const execCalls = [];

  function registerListener(name, callback) {
    listeners[name] = callback;
    return { dispose() {} };
  }

  Module._load = function (request, parent, isMain) {
    if (request === 'vscode') return vscode;
    if (request === './cliDownload') return { ensureReleaseCli: releaseCli };
    if (request === 'child_process') {
      return {
        execFile(executable, args, options, callback) {
          execCalls.push({ executable, args, options });
          const response = result ?? { error: null, stdout: validReport(), stderr: '' };
          callback(response.error, response.stdout, response.stderr);
        },
      };
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  delete Module._cache[extensionModule];
  const extension = Module._load(extensionModule, null, false);
  const context = {
    extension: { packageJSON: { version: '1.2.3' } },
    globalStorageUri: { fsPath: '/extension-storage' },
    subscriptions: [],
  };
  extension.activate(context);
  return { commands, context, diagnostics, execCalls, listeners, messages, output, vscode };
}

function uri(fsPath, scheme = 'file') {
  return { fsPath, scheme, toString: () => `${scheme}:${fsPath}` };
}

function validReport(overrides = {}) {
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

afterEach(() => {
  Module._load = originalLoad;
  delete Module._cache[extensionModule];
});

describe('extension activation and commands', () => {
  it('registers commands and lifecycle listeners', () => {
    const harness = createHarness();

    assert.deepEqual([...harness.commands.keys()], ['papyrusLint.lintFile', 'papyrusLint.fixFile']);
    assert.deepEqual(Object.keys(harness.listeners).sort(), ['close', 'open', 'save']);
    assert.equal(harness.context.subscriptions.length, 7);
  });

  it('lints a selected PSC with config override and publishes normalized diagnostics', async () => {
    const report = validReport({
      files: [{
        path: '/project/Test.psc',
        diagnostics: [{
          line: 3,
          column: 5,
          rule: 'slow-function',
          level: 'warning',
          message: 'Prefer the faster alternative.',
        }],
      }],
    });
    const harness = createHarness({ configPath: '/project/custom.yaml', result: { error: null, stdout: report, stderr: '' } });
    const target = uri('/project/Test.PSC');

    await harness.commands.get('papyrusLint.lintFile')(target);

    assert.deepEqual(harness.execCalls[0], {
      executable: '/tools/PapyrusLinterCLI',
      args: ['--config', '/project/custom.yaml', '--json', '/project/Test.PSC'],
      options: { cwd: '/project', maxBuffer: 10 * 1024 * 1024 },
    });
    const published = harness.diagnostics.published[0][1][0];
    assert.deepEqual(
      { ...published.range },
      { startLine: 2, startColumn: 4, endLine: 2, endColumn: 5 },
    );
    assert.equal(published.severity, 1);
    assert.equal(published.source, 'papyrus-lint');
    assert.equal(published.code, 'slow-function');
  });

  it('uses the active editor and maps information and error severities', async () => {
    const harness = createHarness({
      result: {
        error: null,
        stdout: validReport({
          files: [{
            path: '/project/Test.psc',
            diagnostics: [
              { line: 1, column: 1, rule: 'note', level: 'info', message: 'A note.' },
              { line: 2, column: 2, rule: 'failure', level: 'error', message: 'A failure.' },
            ],
          }],
        }),
        stderr: '',
      },
    });
    harness.vscode.window.activeTextEditor = { document: { uri: uri('/project/Test.psc') } };

    await harness.commands.get('papyrusLint.lintFile')();

    assert.deepEqual(harness.diagnostics.published[0][1].map((entry) => entry.severity), [2, 0]);
  });

  it('saves a dirty open target before fixing and reports the fix count', async () => {
    const harness = createHarness({
      result: { error: Object.assign(new Error('lint findings'), { code: 1 }), stdout: validReport({ files_fixed: 1 }), stderr: '' },
    });
    const target = uri('/project/Test.psc');
    let saves = 0;
    harness.vscode.workspace.textDocuments.push({
      uri: target,
      isDirty: true,
      async save() { saves += 1; return true; },
    });

    await harness.commands.get('papyrusLint.fixFile')(target);

    assert.equal(saves, 1);
    assert.deepEqual(harness.execCalls[0].args, ['fix', '--json', '/project/Test.psc']);
    assert.deepEqual(harness.messages.information, ['Papyrus Lint: fixed Test.psc. 0 issue(s) remain.']);
  });

  it('reports when a file needs no fixes and handles an empty file report', async () => {
    const harness = createHarness({
      result: { error: null, stdout: validReport({ files: [], files_fixed: null }), stderr: '' },
    });

    await harness.commands.get('papyrusLint.fixFile')(uri('/project/Clean.psc'));

    assert.deepEqual(harness.diagnostics.published[0][1], []);
    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: nothing to fix in Clean.psc. 0 issue(s) remain.',
    ]);
  });

  it('downloads the matching CLI when no override is configured', async () => {
    const downloads = [];
    const harness = createHarness({
      cliPath: '   ',
      releaseCli: async (...args) => {
        downloads.push(args);
        return '/downloaded/PapyrusLinterCLI';
      },
    });

    await harness.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.deepEqual(downloads, [['/extension-storage', '1.2.3']]);
    assert.equal(harness.execCalls[0].executable, '/downloaded/PapyrusLinterCLI');
  });

  it('reports download and executable launch failures', async () => {
    const downloadFailure = createHarness({
      cliPath: '',
      releaseCli: async () => { throw new Error('offline'); },
    });
    await downloadFailure.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));
    assert.match(downloadFailure.messages.error[0], /offline/);

    const launchFailure = createHarness({
      result: { error: Object.assign(new Error('not found'), { code: 'ENOENT' }), stdout: '', stderr: '' },
    });
    await launchFailure.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));
    assert.match(launchFailure.messages.error[0], /not found/);
  });

  it('warns instead of running for invalid targets and cancelled saves', async () => {
    const harness = createHarness();
    await harness.commands.get('papyrusLint.lintFile')(uri('/project/readme.txt'));

    const target = uri('/project/Test.psc');
    harness.vscode.workspace.textDocuments.push({ uri: target, isDirty: true, async save() { return false; } });
    await harness.commands.get('papyrusLint.lintFile')(target);

    assert.equal(harness.execCalls.length, 0);
    assert.deepEqual(harness.messages.warning, [
      'Papyrus Lint: open or select a .psc file first.',
      'Papyrus Lint: save the file before linting.',
    ]);
  });

  it('reports usage errors and malformed JSON without replacing diagnostics', async () => {
    const usage = createHarness({ result: { error: Object.assign(new Error('bad arguments'), { code: 2 }), stdout: '', stderr: 'bad arguments\n' } });
    await usage.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));
    assert.deepEqual(usage.output.lines, ['papyrus-lint: bad arguments']);
    assert.deepEqual(usage.messages.error, ['Papyrus Lint: bad arguments']);

    const malformed = createHarness({ result: { error: null, stdout: 'not json', stderr: '' } });
    await malformed.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));
    assert.equal(malformed.diagnostics.published.length, 0);
    assert.match(malformed.messages.error[0], /could not parse the CLI output/);

    await malformed.commands.get('papyrusLint.fixFile')(uri('/project/Test.psc'));
    assert.deepEqual(malformed.messages.information, []);
  });

  it('lints clean Papyrus documents that are already open during activation', async () => {
    const target = uri('/project/AlreadyOpen.psc');
    const harness = createHarness({
      textDocuments: [{ uri: target, languageId: 'papyrus', isDirty: false }],
    });

    await new Promise((resolve) => setImmediate(resolve));

    assert.equal(harness.execCalls.length, 1);
    assert.deepEqual(harness.execCalls[0].args, ['--json', '/project/AlreadyOpen.psc']);
  });

  it('only automatically lints clean Papyrus file documents and clears them on close', async () => {
    const harness = createHarness();
    const target = uri('/project/Test.psc');
    const cleanPapyrus = { uri: target, languageId: 'papyrus', isDirty: false };

    await harness.listeners.open(cleanPapyrus);
    await harness.listeners.save({ ...cleanPapyrus, isDirty: true });
    await harness.listeners.open({ ...cleanPapyrus, languageId: 'plaintext' });
    harness.listeners.close(cleanPapyrus);

    assert.equal(harness.execCalls.length, 1);
    assert.deepEqual(harness.diagnostics.deleted, [target]);
  });

  it('does not clear diagnostics when a non-file Papyrus document closes', () => {
    const harness = createHarness();

    harness.listeners.close({ uri: uri('/project/Test.psc', 'untitled'), languageId: 'papyrus' });

    assert.deepEqual(harness.diagnostics.deleted, []);
  });
});

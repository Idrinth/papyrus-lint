import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri, validReport } from './harness.mjs';

afterEach(restoreModules);

describe('PapyrusLinter', () => {
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
      args: ['--config', '/project/custom.yaml', 'lint', '--format', 'json', '/project/Test.PSC'],
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

  it('publishes a clickable {value, target} code for a diagnostic with a known documentation link', async () => {
    const report = validReport({
      files: [{
        path: '/project/Test.psc',
        diagnostics: [{
          line: 1,
          column: 1,
          rule: 'trailing-whitespace',
          level: 'warning',
          message: 'Trailing whitespace.',
          doc_url: 'https://papyrus-lint.idrinth.de/#lint-trailing-whitespace',
        }],
      }],
    });
    const harness = createHarness({ result: { error: null, stdout: report, stderr: '' } });
    const target = uri('/project/Test.PSC');

    await harness.commands.get('papyrusLint.lintFile')(target);

    const published = harness.diagnostics.published[0][1][0];
    assert.equal(published.code.value, 'trailing-whitespace');
    assert.equal(published.code.target.toString(), 'https://papyrus-lint.idrinth.de/#lint-trailing-whitespace');
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
    assert.deepEqual(harness.execCalls[0].args, ['fix', '--format', 'json', '/project/Test.psc']);
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

  it('fixes a single issue by rule and line, and reports the outcome', async () => {
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

    await harness.commands.get('papyrusLint.fixIssue')(target, 'trailing-whitespace', 3);

    assert.equal(saves, 1);
    assert.deepEqual(harness.execCalls[0].args, [
      'fix', '--type', 'trailing-whitespace', '--line', '3', '--format', 'json', '/project/Test.psc',
    ]);
    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: fixed "trailing-whitespace" on line 3 of Test.psc.',
    ]);
  });

  it('reports when a single issue has no automatic fix', async () => {
    const harness = createHarness({
      result: { error: null, stdout: validReport({ files_fixed: 0 }), stderr: '' },
    });

    await harness.commands.get('papyrusLint.fixIssue')(uri('/project/Test.psc'), 'forbidden-functions', 5);

    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: "forbidden-functions" on line 5 of Test.psc has no automatic fix.',
    ]);
  });

  it('treats a missing fix count as no automatic fix', async () => {
    const harness = createHarness({
      result: { error: null, stdout: validReport({ files_fixed: null }), stderr: '' },
    });

    await harness.commands.get('papyrusLint.fixIssue')(uri('/project/Test.psc'), 'forbidden-functions', 5);

    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: "forbidden-functions" on line 5 of Test.psc has no automatic fix.',
    ]);
  });

  it('does not report a fix outcome when fixing one issue returns malformed JSON', async () => {
    const harness = createHarness({
      result: { error: null, stdout: 'not json', stderr: '' },
    });

    await harness.commands.get('papyrusLint.fixIssue')(
      uri('/project/Test.psc'),
      'trailing-whitespace',
      3,
    );

    assert.deepEqual(harness.messages.information, []);
    assert.match(harness.messages.error[0], /could not parse the CLI output/);
    assert.equal(harness.diagnostics.published.length, 0);
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

  it('rejects a manually configured CLI from a different release', async () => {
    const harness = createHarness({
      versionResult: { error: null, stdout: 'PapyrusLinterCLI 1.2.2\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.equal(harness.execCalls.length, 0);
    assert.match(harness.messages.error[0], /expected "PapyrusLinterCLI 1\.2\.3"/);
    assert.match(harness.messages.error[0], /got "PapyrusLinterCLI 1\.2\.2"/);
  });

  it('reports configured executable verification failures', async () => {
    const errorFailure = createHarness({
      verifyConfiguredExecutable: async () => { throw new Error('untrusted executable'); },
    });
    await errorFailure.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.equal(errorFailure.execCalls.length, 0);
    assert.match(errorFailure.messages.error[0], /untrusted executable/);

    const stringFailure = createHarness({
      verifyConfiguredExecutable: async () => { throw 'verification unavailable'; },
    });
    await stringFailure.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.equal(stringFailure.execCalls.length, 0);
    assert.match(stringFailure.messages.error[0], /verification unavailable/);
  });

  it('describes a configured CLI version check that produces no output', async () => {
    const harness = createHarness({
      versionResult: { error: Object.assign(new Error('failed'), { code: 1 }), stdout: '', stderr: '' },
    });

    await harness.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.equal(harness.execCalls.length, 0);
    assert.match(harness.messages.error[0], /got "no version output"/);
  });

  it('retries an automatic CLI download after a non-Error rejection', async () => {
    let attempts = 0;
    const harness = createHarness({
      cliPath: '',
      releaseCli: async () => {
        attempts += 1;
        if (attempts === 1) throw 'temporarily unavailable';
        return '/downloaded/PapyrusLinterCLI';
      },
    });
    const target = uri('/project/Test.psc');

    await harness.commands.get('papyrusLint.lintFile')(target);
    await harness.commands.get('papyrusLint.lintFile')(target);

    assert.equal(attempts, 2);
    assert.match(harness.messages.error[0], /temporarily unavailable/);
    assert.equal(harness.execCalls.length, 1);
    assert.equal(harness.execCalls[0].executable, '/downloaded/PapyrusLinterCLI');
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

  it('supplies a fallback message for a usage error without stderr', async () => {
    const harness = createHarness({
      result: { error: Object.assign(new Error('bad arguments'), { code: 2 }), stdout: '', stderr: '  ' },
    });

    await harness.commands.get('papyrusLint.lintFile')(uri('/project/Test.psc'));

    assert.deepEqual(harness.output.lines, ['papyrus-lint: failed to lint file.']);
    assert.deepEqual(harness.messages.error, ['Papyrus Lint: failed to lint file.']);
  });
});

describe('workspace-root config auto-detection', () => {
  let workspaceRoot;

  afterEach(async () => {
    if (workspaceRoot) {
      await rm(workspaceRoot, { recursive: true, force: true });
      workspaceRoot = undefined;
    }
  });

  it('passes --config for a papyrus-lint.yaml found at the workspace root via getWorkspaceFolder, even when the script sits deeper under a Data/Scripts/Source subfolder', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-'));
    const scriptDir = path.join(workspaceRoot, 'Data', 'Scripts', 'Source');
    await mkdir(scriptDir, { recursive: true });
    const configFile = path.join(workspaceRoot, 'papyrus-lint.yaml');
    await writeFile(configFile, 'semicolon: true\n');
    const scriptPath = path.join(scriptDir, 'Test.psc');

    const harness = createHarness({ workspaceFolders: [{ uri: uri(workspaceRoot) }] });

    await harness.commands.get('papyrusLint.lintFile')(uri(scriptPath));

    assert.deepEqual(harness.execCalls[0].args, ['--config', configFile, 'lint', '--format', 'json', scriptPath]);
  });

  it('leaves args untouched when the document is outside every open workspace folder', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-'));
    await writeFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'semicolon: true\n');

    const harness = createHarness({ workspaceFolders: [{ uri: uri(workspaceRoot) }] });

    await harness.commands.get('papyrusLint.lintFile')(uri('/elsewhere/Test.psc'));

    assert.deepEqual(harness.execCalls[0].args, ['lint', '--format', 'json', '/elsewhere/Test.psc']);
  });

  it('leaves args untouched when no config file exists anywhere in the workspace folder', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-'));
    const scriptPath = path.join(workspaceRoot, 'Test.psc');

    const harness = createHarness({ workspaceFolders: [{ uri: uri(workspaceRoot) }] });

    await harness.commands.get('papyrusLint.lintFile')(uri(scriptPath));

    assert.deepEqual(harness.execCalls[0].args, ['lint', '--format', 'json', scriptPath]);
  });

  it('still prefers the explicit papyrusLint.configPath setting over an auto-detected workspace config', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-'));
    await writeFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'semicolon: true\n');
    const scriptPath = path.join(workspaceRoot, 'Test.psc');

    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      configPath: '/explicit/override.yaml',
    });

    await harness.commands.get('papyrusLint.lintFile')(uri(scriptPath));

    assert.deepEqual(harness.execCalls[0].args, ['--config', '/explicit/override.yaml', 'lint', '--format', 'json', scriptPath]);
  });
});

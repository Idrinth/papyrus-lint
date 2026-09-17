import assert from 'node:assert/strict';
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
      'fix', '--type', 'trailing-whitespace', '--line', '3', '--json', '/project/Test.psc',
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

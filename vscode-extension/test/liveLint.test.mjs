import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, it } from 'node:test';
import { createHarness, papyrusDocument, restoreModules, uri, validReport } from './harness.mjs';

afterEach(restoreModules);

describe('live linting via --blob', () => {
  it('lints a document\'s current contents on change, after debouncing', async () => {
    const harness = createHarness({ configPath: '/project/custom.yaml' });
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test   \n');

    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.deepEqual(harness.execCalls[0], {
      executable: '/tools/PapyrusLinterCLI',
      args: ['--config', '/project/custom.yaml', 'lint', '--format', 'json', '--blob', 'ScriptName Test   \n'],
      options: { cwd: '/project', maxBuffer: 10 * 1024 * 1024 },
    });
    assert.equal(harness.diagnostics.published[0][0], document.uri);
  });

  it('coalesces rapid successive changes into a single debounced lint', async () => {
    const harness = createHarness();
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document: papyrusDocument('/project/Test.psc', 'ScriptName Te\n') });
    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 1);
    assert.deepEqual(harness.execCalls[0].args, ['lint', '--format', 'json', '--blob', 'ScriptName Test\n']);
  });

  it('does not let an older in-flight lint overwrite a newer result', async () => {
    const callbacks = [];
    const harness = createHarness({
      result: ({ callback }) => callbacks.push(callback),
    });
    const older = papyrusDocument('/project/Test.psc', 'ScriptName Older\n');
    const newer = papyrusDocument('/project/Test.psc', 'ScriptName Newer\n');

    harness.listeners.change({ document: older });
    await new Promise((resolve) => setTimeout(resolve, 0));
    harness.listeners.change({ document: newer });
    await new Promise((resolve) => setTimeout(resolve, 0));

    callbacks[1](null, validReport({
      files: [{
        path: '/project/Test.psc',
        diagnostics: [{ line: 2, column: 1, rule: 'newer', message: 'Newer result.' }],
      }],
    }), '');
    await new Promise((resolve) => setTimeout(resolve, 0));
    callbacks[0](null, validReport({
      files: [{
        path: '/project/Test.psc',
        diagnostics: [{ line: 1, column: 1, rule: 'older', message: 'Older result.' }],
      }],
    }), '');
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.diagnostics.published.length, 1);
    assert.equal(harness.diagnostics.published[0][1][0].code, 'newer');
  });

  it('logs a live lint failure instead of showing an error message', async () => {
    const harness = createHarness({
      result: { error: Object.assign(new Error('bad arguments'), { code: 2 }), stdout: '', stderr: 'bad arguments\n' },
    });
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.deepEqual(harness.output.lines, ['papyrus-lint: bad arguments']);
    assert.deepEqual(harness.messages.error, []);
  });

  it('does not live lint non-Papyrus or non-file documents', async () => {
    const harness = createHarness();

    harness.listeners.change({ document: { ...papyrusDocument('/project/Test.psc', 'x'), languageId: 'plaintext' } });
    harness.listeners.change({ document: papyrusDocument('/project/Test.psc', 'x') });
    harness.listeners.change({ document: { uri: uri('/project/Test.psc', 'untitled'), languageId: 'papyrus', getText: () => 'x' } });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 1);
  });

  it('can be disabled via the liveLint setting', async () => {
    const harness = createHarness({ liveLint: false });
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 0);
  });

  it('cancels a pending live lint when the document closes first', async () => {
    const harness = createHarness();
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    harness.listeners.close(document);
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 0);
  });

  it('does not republish diagnostics when an in-flight lint finishes after close', async () => {
    let finishLint;
    const harness = createHarness({
      result: ({ callback }) => { finishLint = callback; },
    });
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));
    harness.listeners.close(document);
    finishLint(null, validReport({
      files: [{
        path: '/project/Test.psc',
        diagnostics: [{ line: 1, column: 1, rule: 'late', message: 'Late result.' }],
      }],
    }), '');
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.diagnostics.deleted.length, 1);
    assert.equal(harness.diagnostics.published.length, 0);
  });

  it('clears pending live lints on deactivate', async () => {
    const harness = createHarness();
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    harness.extension.deactivate();
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 0);
  });

  it('picks up a papyrus-lint.yaml at the workspace root via getWorkspaceFolder, unlike the CLI\'s own --blob discovery', async () => {
    const workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-'));
    try {
      await writeFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'semicolon: true\n');
      const scriptPath = path.join(workspaceRoot, 'Test.psc');
      const harness = createHarness({ workspaceFolders: [{ uri: uri(workspaceRoot) }] });
      const document = papyrusDocument(scriptPath, 'ScriptName Test\n');

      harness.listeners.change({ document });
      // Unlike the other harness-only cases above, resolving this config path walks
      // the real filesystem (see findConfigInWorkspace in config.ts), so it needs more
      // than a single macrotask tick to settle before the debounced lint fires.
      await new Promise((resolve) => setTimeout(resolve, 50));

      assert.deepEqual(harness.execCalls[0].args, [
        '--config',
        path.join(workspaceRoot, 'papyrus-lint.yaml'),
        'lint', '--format', 'json',
        '--blob',
        'ScriptName Test\n',
      ]);
    } finally {
      await rm(workspaceRoot, { recursive: true, force: true });
    }
  });
});

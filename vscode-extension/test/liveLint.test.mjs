import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, papyrusDocument, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('live linting via --blob', () => {
  it('lints a document\'s current contents on change, after debouncing', async () => {
    const harness = createHarness({ configPath: '/project/custom.yaml' });
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test   \n');

    harness.listeners.change({ document });
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.deepEqual(harness.execCalls[0], {
      executable: '/tools/PapyrusLinterCLI',
      args: ['--config', '/project/custom.yaml', '--json', '--blob', 'ScriptName Test   \n'],
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
    assert.deepEqual(harness.execCalls[0].args, ['--json', '--blob', 'ScriptName Test\n']);
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

  it('clears pending live lints on deactivate', async () => {
    const harness = createHarness();
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Test\n');

    harness.listeners.change({ document });
    harness.extension.deactivate();
    await new Promise((resolve) => setTimeout(resolve, 0));

    assert.equal(harness.execCalls.length, 0);
  });
});

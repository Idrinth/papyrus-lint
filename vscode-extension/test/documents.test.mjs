import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('resolveTargetUri', () => {
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
});

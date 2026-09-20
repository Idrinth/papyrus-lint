import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('extension activation', () => {
  it('registers commands and lifecycle listeners', () => {
    const harness = createHarness();

    assert.deepEqual(
      [...harness.commands.keys()],
      [
        'papyrusLint.lintFile',
        'papyrusLint.fixFile',
        'papyrusLint.fixIssue',
        'papyrusLint.ignoreIssueForLine',
        'papyrusLint.ignoreIssueForFile',
        'papyrusLint.ignoreIssueForProject',
        'papyrusLint.initializeConfig',
      ],
    );
    assert.deepEqual(Object.keys(harness.listeners).sort(), ['change', 'close', 'open', 'save']);
    assert.equal(harness.context.subscriptions.length, 14);
    assert.equal(harness.codeActionProviders.length, 1);
    assert.equal(harness.extension.deactivate(), undefined);
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
    await new Promise((resolve) => setImmediate(resolve));
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

  it('starts downloading the matching CLI on activation when no override is configured', async () => {
    const downloads = [];
    createHarness({
      cliPath: '',
      releaseCli: async (...args) => {
        downloads.push(args);
        return '/downloaded/PapyrusLinterCLI';
      },
    });

    assert.deepEqual(downloads, [['/extension-storage', '1.2.3']]);
  });

  it('does not download a CLI on activation when papyrusLint.cliPath is set', () => {
    const downloads = [];
    createHarness({
      releaseCli: async (...args) => {
        downloads.push(args);
        return '/downloaded/PapyrusLinterCLI';
      },
    });

    assert.deepEqual(downloads, []);
  });
});

import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

function cleanPapyrusDocument(fsPath) {
  return { uri: uri(fsPath), languageId: 'papyrus', isDirty: false };
}

describe('resource-scoped settings in a multi-root workspace', () => {
  it('resolves papyrusLint.configPath per workspace folder', async () => {
    const folderA = uri('/root/folderA');
    const folderB = uri('/root/folderB');
    const harness = createHarness({
      workspaceFolders: [{ uri: folderA }, { uri: folderB }],
      folderConfig: {
        '/root/folderA': { configPath: '/root/folderA/custom.yaml' },
        '/root/folderB': { configPath: '/root/folderB/other.yaml' },
      },
    });

    await harness.listeners.open(cleanPapyrusDocument('/root/folderA/Test.psc'));
    await new Promise((resolve) => setImmediate(resolve));
    await harness.listeners.open(cleanPapyrusDocument('/root/folderB/Test.psc'));
    await new Promise((resolve) => setImmediate(resolve));

    assert.deepEqual(harness.execCalls[0].args, [
      'lint', '--config', '/root/folderA/custom.yaml', '--format', 'json', '/root/folderA/Test.psc',
    ]);
    assert.deepEqual(harness.execCalls[1].args, [
      'lint', '--config', '/root/folderB/other.yaml', '--format', 'json', '/root/folderB/Test.psc',
    ]);
  });

  it('resolves papyrusLint.liveLint per workspace folder', async () => {
    const folderA = uri('/root/folderA');
    const folderB = uri('/root/folderB');
    const harness = createHarness({
      workspaceFolders: [{ uri: folderA }, { uri: folderB }],
      folderConfig: {
        '/root/folderA': { liveLint: false, configPath: '/root/folderA/custom.yaml' },
        '/root/folderB': { liveLint: true, configPath: '/root/folderB/other.yaml' },
      },
    });

    harness.listeners.change({
      document: { ...cleanPapyrusDocument('/root/folderA/Test.psc'), getText: () => 'ScriptName A\n' },
    });
    harness.listeners.change({
      document: { ...cleanPapyrusDocument('/root/folderB/Test.psc'), getText: () => 'ScriptName B\n' },
    });
    await new Promise((resolve) => setTimeout(resolve, 20));

    assert.equal(harness.execCalls.length, 1);
    assert.deepEqual(harness.execCalls[0].args, [
      'lint', '--config', '/root/folderB/other.yaml', '--format', 'json', '--blob', 'ScriptName B\n',
    ]);
  });

  it('resolves papyrusLint.liveLintDebounceMs per workspace folder', async () => {
    const folderA = uri('/root/folderA');
    const folderB = uri('/root/folderB');
    const harness = createHarness({
      workspaceFolders: [{ uri: folderA }, { uri: folderB }],
      folderConfig: {
        '/root/folderA': { liveLintDebounceMs: 100, configPath: '/root/folderA/custom.yaml' },
        '/root/folderB': { liveLintDebounceMs: 0, configPath: '/root/folderB/other.yaml' },
      },
    });

    harness.listeners.change({
      document: { ...cleanPapyrusDocument('/root/folderA/Test.psc'), getText: () => 'ScriptName A\n' },
    });
    harness.listeners.change({
      document: { ...cleanPapyrusDocument('/root/folderB/Test.psc'), getText: () => 'ScriptName B\n' },
    });
    await new Promise((resolve) => setTimeout(resolve, 20));

    // The short-debounce folder's lint fires promptly; the long-debounce
    // folder's is still pending.
    assert.equal(harness.execCalls.length, 1);
    assert.equal(harness.execCalls[0].options.cwd, '/root/folderB');

    await new Promise((resolve) => setTimeout(resolve, 120));

    assert.equal(harness.execCalls.length, 2);
    assert.equal(harness.execCalls[1].options.cwd, '/root/folderA');
  });

  it('falls back to the window-wide setting for a document outside any workspace folder', async () => {
    const harness = createHarness({
      configPath: '/global/papyrus-lint.yaml',
      workspaceFolders: [{ uri: uri('/root/folderA') }],
      folderConfig: {
        '/root/folderA': { configPath: '/root/folderA/custom.yaml' },
      },
    });

    await harness.listeners.open(cleanPapyrusDocument('/outside/Test.psc'));
    await new Promise((resolve) => setImmediate(resolve));

    assert.deepEqual(harness.execCalls[0].args, [
      'lint', '--config', '/global/papyrus-lint.yaml', '--format', 'json', '/outside/Test.psc',
    ]);
  });
});

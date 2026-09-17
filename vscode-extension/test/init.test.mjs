import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('papyrusLint.initializeConfig', () => {
  it('initializes the sole workspace folder with the default preset', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'strict (default)', preset: '' },
      result: { error: null, stdout: 'Created /project/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.execCalls[0], {
      executable: '/tools/PapyrusLinterCLI',
      args: ['init'],
      options: { cwd: '/project', maxBuffer: 10 * 1024 * 1024 },
    });
    assert.deepEqual(harness.messages.information, ['Papyrus Lint: Created /project/papyrus-lint.yaml']);
  });

  it('passes a chosen built-in preset', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'careful', preset: 'careful' },
      result: { error: null, stdout: 'Created /project/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.execCalls[0].args, ['init', '--preset', 'careful']);
  });

  it('uses the default preset when a built-in quick-pick item omits its value', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'strict (default)' },
      result: { error: null, stdout: 'Created /project/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.execCalls[0].args, ['init']);
  });

  it('prompts for and passes a custom preset name', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'Custom preset name…' },
      inputBoxResult: '  team-style  ',
      result: { error: null, stdout: 'Created /project/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.execCalls[0].args, ['init', '--preset', 'team-style']);
  });

  it('prompts for a workspace folder when several are open', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/one') }, { uri: uri('/two') }],
      workspaceFolderPickResult: { uri: uri('/two') },
      quickPickResult: { label: 'strict (default)', preset: '' },
      result: { error: null, stdout: 'Created /two/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.equal(harness.execCalls[0].options.cwd, '/two');
  });

  it('initializes a right-clicked explorer folder directly, without prompting for one', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/one') }, { uri: uri('/two') }],
      quickPickResult: { label: 'strict (default)', preset: '' },
      result: { error: null, stdout: 'Created /three/papyrus-lint.yaml\n', stderr: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')(uri('/three'));

    assert.equal(harness.execCalls[0].options.cwd, '/three');
  });

  it('shows an error and does nothing when no folder is open or picked', async () => {
    const noFolders = createHarness({ workspaceFolders: undefined });
    await noFolders.commands.get('papyrusLint.initializeConfig')();
    assert.deepEqual(noFolders.messages.error, ['Papyrus Lint: open a folder or workspace first.']);
    assert.equal(noFolders.execCalls.length, 0);

    const cancelledPick = createHarness({
      workspaceFolders: [{ uri: uri('/one') }, { uri: uri('/two') }],
      workspaceFolderPickResult: undefined,
    });
    await cancelledPick.commands.get('papyrusLint.initializeConfig')();
    assert.equal(cancelledPick.execCalls.length, 0);
  });

  it('does nothing when the preset prompt is cancelled at either step', async () => {
    const cancelledQuickPick = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: undefined,
    });
    await cancelledQuickPick.commands.get('papyrusLint.initializeConfig')();
    assert.equal(cancelledQuickPick.execCalls.length, 0);

    const cancelledCustomInput = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'Custom preset name…' },
      inputBoxResult: '   ',
    });
    await cancelledCustomInput.commands.get('papyrusLint.initializeConfig')();
    assert.equal(cancelledCustomInput.execCalls.length, 0);
  });

  it('reports a failure to initialize (e.g. an existing config)', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'strict (default)', preset: '' },
      result: {
        error: Object.assign(new Error('usage'), { code: 2 }),
        stdout: '',
        stderr: 'error: failed to initialize config: papyrus-lint.yaml already exists\n',
      },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.output.lines, [
      'papyrus-lint: error: failed to initialize config: papyrus-lint.yaml already exists',
    ]);
    assert.deepEqual(harness.messages.error, [
      'Papyrus Lint: error: failed to initialize config: papyrus-lint.yaml already exists',
    ]);
    assert.deepEqual(harness.messages.information, []);
  });

  it('supplies a fallback message when initialization fails without stderr', async () => {
    const harness = createHarness({
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'strict (default)', preset: '' },
      result: { error: Object.assign(new Error('usage'), { code: 2 }), stdout: '', stderr: '  ' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.deepEqual(harness.output.lines, ['papyrus-lint: failed to initialize config.']);
    assert.deepEqual(harness.messages.error, ['Papyrus Lint: failed to initialize config.']);
  });

  it('reports a CLI launch failure the same way as linting does', async () => {
    const harness = createHarness({
      cliPath: '',
      releaseCli: async () => { throw new Error('offline'); },
      workspaceFolders: [{ uri: uri('/project') }],
      quickPickResult: { label: 'strict (default)', preset: '' },
    });

    await harness.commands.get('papyrusLint.initializeConfig')();

    assert.match(harness.messages.error[0], /offline/);
  });
});

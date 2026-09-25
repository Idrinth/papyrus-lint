import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, it } from 'node:test';
import { createHarness, papyrusDocument, restoreModules, uri, validReport } from './harness.mjs';

afterEach(restoreModules);

describe('papyrusLint.ignoreIssueForLine', () => {
  it('inserts @disable on the diagnostic line and re-lints the unsaved buffer via --blob', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Example\nCall(1,2)\n');
    const harness = createHarness({ textDocuments: [document] });

    await harness.commands.get('papyrusLint.ignoreIssueForLine')(target, 'comma-spacing', 2);

    assert.equal(document.getText(), 'ScriptName Example\nCall(1,2) ; @disable comma-spacing\n');
    assert.equal(document.isDirty, true);
    assert.equal(harness.appliedEdits.length, 1);
    assert.match(harness.execCalls.at(-1).args.join(' '), /--blob/);
    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: ignoring "comma-spacing" on line 2 of Test.psc.',
    ]);
  });

  it('still re-lints when the line already disables the rule', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', 'Call(1,2) ; @disable comma-spacing\n');
    const harness = createHarness({ textDocuments: [document] });

    await harness.commands.get('papyrusLint.ignoreIssueForLine')(target, 'comma-spacing', 1);

    assert.equal(harness.appliedEdits.length, 0);
    assert.match(harness.execCalls[0].args.join(' '), /--blob/);
  });

  it('reports when the workspace edit cannot be applied', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', 'Call(1,2)\n');
    const harness = createHarness({ textDocuments: [document], applyEditResult: false });

    await harness.commands.get('papyrusLint.ignoreIssueForLine')(target, 'comma-spacing', 1);

    assert.match(harness.messages.error[0], /could not add @disable/);
    assert.equal(harness.execCalls.length, 0);
  });

  it('reports when the script cannot be opened', async () => {
    const harness = createHarness();

    await harness.commands.get('papyrusLint.ignoreIssueForLine')(uri('/project/Missing.psc'), 'comma-spacing', 1);

    assert.match(harness.messages.error[0], /could not open Missing.psc/);
  });

  it('opens the script when it is not already in the editor', async () => {
    const document = papyrusDocument('/project/Test.psc', 'Call(1,2)\n');
    const harness = createHarness();
    harness.vscode.workspace.openTextDocument = async () => {
      harness.vscode.workspace.textDocuments.push(document);
      return document;
    };

    await harness.commands.get('papyrusLint.ignoreIssueForLine')(document.uri, 'comma-spacing', 1);

    assert.equal(document.getText(), 'Call(1,2) ; @disable comma-spacing\n');
    assert.match(harness.execCalls.at(-1).args.join(' '), /--blob/);
  });
});

describe('papyrusLint.ignoreIssueForFile', () => {
  it('inserts @disable-file and re-lints the unsaved buffer via --blob', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Example\n');
    const harness = createHarness({ textDocuments: [document] });

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(target, 'trailing-whitespace');

    assert.equal(document.getText(), '; @disable-file trailing-whitespace\nScriptName Example\n');
    assert.equal(document.isDirty, true);
    assert.equal(harness.appliedEdits.length, 1);
    assert.match(harness.execCalls.at(-1).args.join(' '), /--blob/);
    assert.deepEqual(harness.messages.information, [
      'Papyrus Lint: ignoring "trailing-whitespace" for Test.psc.',
    ]);
  });

  it('still re-lints when the file already disables the rule', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', '; @disable-file trailing-whitespace\nScriptName Example\n');
    const harness = createHarness({ textDocuments: [document] });

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(target, 'trailing-whitespace');

    assert.equal(harness.appliedEdits.length, 0);
    assert.match(harness.execCalls[0].args.join(' '), /--blob/);
  });

  it('reports when the workspace edit cannot be applied', async () => {
    const target = uri('/project/Test.psc');
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Example\n');
    const harness = createHarness({ textDocuments: [document], applyEditResult: false });

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(target, 'trailing-whitespace');

    assert.match(harness.messages.error[0], /could not add @disable-file/);
    assert.equal(harness.execCalls.length, 0);
  });

  it('reports when the script cannot be opened', async () => {
    const harness = createHarness();

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(uri('/project/Missing.psc'), 'trailing-whitespace');

    assert.match(harness.messages.error[0], /could not open Missing.psc/);
  });

  it('opens the script when it is not already in the editor', async () => {
    const document = papyrusDocument('/project/Test.psc', 'ScriptName Example\n');
    const harness = createHarness();
    harness.vscode.workspace.openTextDocument = async () => {
      harness.vscode.workspace.textDocuments.push(document);
      return document;
    };

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(document.uri, 'trailing-whitespace');

    assert.equal(document.getText(), '; @disable-file trailing-whitespace\nScriptName Example\n');
    assert.match(harness.execCalls.at(-1).args.join(' '), /--blob/);
  });

  it('stringifies a non-Error thrown while opening the script', async () => {
    const harness = createHarness();
    harness.vscode.workspace.openTextDocument = async () => {
      throw 'missing';
    };

    await harness.commands.get('papyrusLint.ignoreIssueForFile')(uri('/project/Test.psc'), 'trailing-whitespace');

    assert.match(harness.messages.error[0], /could not open Test.psc \(missing\)/);
  });
});

describe('papyrusLint.ignoreIssueForProject', () => {
  let workspaceRoot;

  afterEach(async () => {
    if (workspaceRoot) {
      await rm(workspaceRoot, { recursive: true, force: true });
      workspaceRoot = undefined;
    }
  });

  it('creates papyrus-lint.yaml when the workspace has no config yet', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const scriptPath = path.join(workspaceRoot, 'Test.psc');
    const document = papyrusDocument(scriptPath, 'ScriptName Example\n');
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      textDocuments: [document],
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(scriptPath), 'trailing-whitespace');

    const written = await readFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'utf8');
    assert.equal(written, 'rules:\n  trailing_whitespace: false\n');
    assert.match(harness.execCalls.at(-1).args.join(' '), /--config/);
    assert.match(harness.messages.information[0], /ignoring "trailing-whitespace" for the project/);
  });

  it('flips an existing rules toggle in papyrus-lint.yml', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const configFile = path.join(workspaceRoot, 'papyrus-lint.yml');
    await writeFile(configFile, 'rules:\n  trailing_whitespace: true\n  comma_spacing: true\n');
    const scriptPath = path.join(workspaceRoot, 'Test.psc');
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(scriptPath), 'trailing-whitespace');

    assert.equal(
      await readFile(configFile, 'utf8'),
      'rules:\n  trailing_whitespace: false\n  comma_spacing: true\n',
    );
  });

  it('edits an already-open config document and saves it', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const configFile = path.join(workspaceRoot, 'papyrus-lint.yaml');
    await writeFile(configFile, 'rules:\n  comma_spacing: true\n');
    const configDocument = {
      uri: uri(configFile),
      languageId: 'yaml',
      isDirty: true,
      getText: () => 'rules:\n  comma_spacing: true\n',
      positionAt: (offset) => ({ line: 0, character: offset }),
      async save() {
        this.saved = true;
        return true;
      },
    };
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      textDocuments: [configDocument],
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(path.join(workspaceRoot, 'Test.psc')), 'trailing-whitespace');

    assert.equal(configDocument.getText(), 'rules:\n  trailing_whitespace: false\n  comma_spacing: true\n');
    assert.equal(configDocument.saved, true);
    assert.equal(harness.appliedEdits.length, 1);
  });

  it('uses papyrusLint.configPath even when that file does not exist yet', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const override = path.join(workspaceRoot, 'custom.yaml');
    const harness = createHarness({
      configPath: override,
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(path.join(workspaceRoot, 'Test.psc')), 'comma-spacing');

    assert.equal(await readFile(override, 'utf8'), 'rules:\n  comma_spacing: false\n');
  });

  it('reports when no folder is open to write a config into', async () => {
    const harness = createHarness({ workspaceFolders: undefined });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri('/project/Test.psc'), 'trailing-whitespace');

    assert.match(harness.messages.error[0], /open a folder or set papyrusLint.configPath/);
    assert.equal(harness.execCalls.length, 0);
  });

  it('reports when an open config cannot be saved', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const configFile = path.join(workspaceRoot, 'papyrus-lint.yaml');
    await writeFile(configFile, 'semicolon: true\n');
    const configDocument = {
      uri: uri(configFile),
      languageId: 'yaml',
      isDirty: true,
      getText: () => 'semicolon: true\n',
      positionAt: (offset) => ({ line: 0, character: offset }),
      async save() { return false; },
    };
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      textDocuments: [configDocument],
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(path.join(workspaceRoot, 'Test.psc')), 'trailing-whitespace');

    assert.match(harness.messages.error[0], /could not disable "trailing-whitespace"/);
    assert.equal(harness.execCalls.length, 0);
  });

  it('reports when an open config cannot be edited', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    const configFile = path.join(workspaceRoot, 'papyrus-lint.yaml');
    await writeFile(configFile, 'semicolon: true\n');
    const configDocument = {
      uri: uri(configFile),
      languageId: 'yaml',
      isDirty: true,
      getText: () => 'semicolon: true\n',
      positionAt: (offset) => ({ line: 0, character: offset }),
      async save() { return true; },
    };
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      textDocuments: [configDocument],
      applyEditResult: false,
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(path.join(workspaceRoot, 'Test.psc')), 'trailing-whitespace');

    assert.match(harness.messages.error[0], /could not disable "trailing-whitespace"/);
  });

  it('re-lints a dirty open script via --blob after changing the project config', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    await writeFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'semicolon: true\n');
    const scriptPath = path.join(workspaceRoot, 'Test.psc');
    const document = papyrusDocument(scriptPath, 'ScriptName Example  \n');
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
      textDocuments: [document],
      result: { error: null, stdout: validReport(), stderr: '' },
    });

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(scriptPath), 'trailing-whitespace');

    assert.match(harness.execCalls[0].args.join(' '), /--blob/);
  });

  it('re-lints a saved open script from disk after changing the project config', async () => {
    workspaceRoot = await mkdtemp(path.join(tmpdir(), 'papyrus-lint-ignore-'));
    await writeFile(path.join(workspaceRoot, 'papyrus-lint.yaml'), 'semicolon: true\n');
    const scriptPath = path.join(workspaceRoot, 'Test.psc');
    const document = papyrusDocument(scriptPath, 'ScriptName Example\n');
    document.isDirty = false;
    const harness = createHarness({
      workspaceFolders: [{ uri: uri(workspaceRoot) }],
    });
    harness.vscode.workspace.textDocuments.push(document);

    await harness.commands.get('papyrusLint.ignoreIssueForProject')(uri(scriptPath), 'trailing-whitespace');

    assert.deepEqual(harness.execCalls.at(-1).args, [
      'lint', '--config',
      path.join(workspaceRoot, 'papyrus-lint.yaml'),
      '--format', 'json',
      scriptPath,
    ]);
  });
});

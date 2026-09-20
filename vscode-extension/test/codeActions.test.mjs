import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('PapyrusFixIssueActionProvider', () => {
  it('offers a quick fix and file/project ignore actions for each papyrus-lint diagnostic', () => {
    const harness = createHarness();
    const [{ provider }] = harness.codeActionProviders;
    const target = uri('/project/Test.psc');
    const papyrusLintDiagnostic = {
      source: 'papyrus-lint',
      code: 'trailing-whitespace',
      range: { start: { line: 2 } },
    };
    const otherSourceDiagnostic = { source: 'other-linter', code: 'rule', range: { start: { line: 0 } } };
    const numericCodeDiagnostic = { source: 'papyrus-lint', code: 123, range: { start: { line: 1 } } };
    const compilerDiagnostic = { source: 'papyrus-lint', code: 'compiler-error', range: { start: { line: 6 } } };
    const linkedDiagnostic = {
      source: 'papyrus-lint',
      code: { value: 'comma-spacing', target: harness.vscode.Uri.parse('https://papyrus-lint.idrinth.de/#lint-space-after-comma') },
      range: { start: { line: 4 } },
    };
    const context = {
      diagnostics: [papyrusLintDiagnostic, otherSourceDiagnostic, numericCodeDiagnostic, compilerDiagnostic, linkedDiagnostic],
    };

    const actions = provider.provideCodeActions({ uri: target, getText: () => 'ScriptName Example\n' }, {}, context);

    assert.equal(actions.length, 7);
    assert.equal(actions[0].title, 'Fix this issue (trailing-whitespace)');
    assert.equal(actions[0].kind, harness.vscode.CodeActionKind.QuickFix);
    assert.deepEqual(actions[0].diagnostics, [papyrusLintDiagnostic]);
    assert.deepEqual(actions[0].command, {
      command: 'papyrusLint.fixIssue',
      title: 'Fix this issue (trailing-whitespace)',
      arguments: [target, 'trailing-whitespace', 3],
    });
    assert.equal(actions[1].title, 'Ignore this lint for the file (trailing-whitespace)');
    assert.deepEqual(actions[1].command, {
      command: 'papyrusLint.ignoreIssueForFile',
      title: 'Ignore this lint for the file (trailing-whitespace)',
      arguments: [target, 'trailing-whitespace'],
    });
    assert.equal(actions[2].title, 'Ignore this lint for the project (trailing-whitespace)');
    assert.deepEqual(actions[2].command.arguments, [target, 'trailing-whitespace']);
    assert.equal(actions[3].title, 'Fix this issue (compiler-error)');
    assert.equal(actions[4].title, 'Fix this issue (comma-spacing)');
    assert.deepEqual(actions[4].command.arguments, [target, 'comma-spacing', 5]);
    assert.equal(actions[5].title, 'Ignore this lint for the file (comma-spacing)');
    assert.equal(actions[6].title, 'Ignore this lint for the project (comma-spacing)');
  });

  it('omits the file-ignore action when @disable-file already covers the rule', () => {
    const harness = createHarness();
    const [{ provider }] = harness.codeActionProviders;
    const target = uri('/project/Test.psc');
    const diagnostic = {
      source: 'papyrus-lint',
      code: 'trailing-whitespace',
      range: { start: { line: 0 } },
    };

    const actions = provider.provideCodeActions(
      { uri: target, getText: () => '; @disable-file trailing-whitespace\nScriptName Example\n' },
      {},
      { diagnostics: [diagnostic] },
    );

    assert.deepEqual(actions.map((action) => action.title), [
      'Fix this issue (trailing-whitespace)',
      'Ignore this lint for the project (trailing-whitespace)',
    ]);
  });
});

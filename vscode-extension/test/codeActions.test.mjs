import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, restoreModules, uri } from './harness.mjs';

afterEach(restoreModules);

describe('PapyrusFixIssueActionProvider', () => {
  it('offers a quick fix command for each papyrus-lint diagnostic under the cursor', () => {
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
    const linkedDiagnostic = {
      source: 'papyrus-lint',
      code: { value: 'comma-spacing', target: harness.vscode.Uri.parse('https://papyrus-lint.idrinth.de/#lint-space-after-comma') },
      range: { start: { line: 4 } },
    };
    const context = {
      diagnostics: [papyrusLintDiagnostic, otherSourceDiagnostic, numericCodeDiagnostic, linkedDiagnostic],
    };

    const actions = provider.provideCodeActions({ uri: target }, {}, context);

    assert.equal(actions.length, 2);
    assert.equal(actions[0].title, 'Fix this issue (trailing-whitespace)');
    assert.equal(actions[0].kind, harness.vscode.CodeActionKind.QuickFix);
    assert.deepEqual(actions[0].diagnostics, [papyrusLintDiagnostic]);
    assert.deepEqual(actions[0].command, {
      command: 'papyrusLint.fixIssue',
      title: 'Fix this issue (trailing-whitespace)',
      arguments: [target, 'trailing-whitespace', 3],
    });
    // A diagnostic whose code carries a documentation link (an object rather
    // than a plain string) still resolves to its own rule id.
    assert.equal(actions[1].title, 'Fix this issue (comma-spacing)');
    assert.deepEqual(actions[1].command.arguments, [target, 'comma-spacing', 5]);
  });
});

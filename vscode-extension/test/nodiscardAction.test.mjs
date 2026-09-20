import assert from 'node:assert/strict';
import { afterEach, describe, it } from 'node:test';
import { createHarness, papyrusDocument, restoreModules } from './harness.mjs';

afterEach(restoreModules);

describe('PapyrusNodiscardActionProvider', () => {
  it('offers "Add ; @nodiscard flag" on an eligible, unflagged header', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Int Function GetValue()\n    Return 1\nEndFunction\n');

    const actions = provider.provideCodeActions(document, { start: { line: 0 } });

    assert.equal(actions.length, 1);
    assert.equal(actions[0].title, 'Add ; @nodiscard flag');
    assert.equal(actions[0].kind, harness.vscode.CodeActionKind.RefactorRewrite);
    assert.deepEqual(actions[0].edit.inserts, [
      { uri: document.uri, position: { line: 0, character: 'Int Function GetValue()'.length }, newText: ' ; @nodiscard' },
    ]);
  });

  it('offers the action for a Native function with no return value', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Function DoThing() Native\n');

    const actions = provider.provideCodeActions(document, { start: { line: 0 } });

    assert.equal(actions.length, 1);
  });

  it('offers nothing for a void, non-native function', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Function DoThing()\nEndFunction\n');

    assert.deepEqual(provider.provideCodeActions(document, { start: { line: 0 } }), []);
  });

  it('offers nothing when the header is already flagged', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Int Function GetValue() ; @nodiscard\n');

    assert.deepEqual(provider.provideCodeActions(document, { start: { line: 0 } }), []);
  });

  it('offers nothing when the cursor is not on a function header', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Int Function GetValue()\n    Return 1\nEndFunction\n');

    assert.deepEqual(provider.provideCodeActions(document, { start: { line: 1 } }), []);
  });

  it('extends an existing trailing comment instead of starting a second one', () => {
    const harness = createHarness();
    const [, { provider }] = harness.codeActionProviders;
    const document = papyrusDocument('/project/Test.psc', 'Int Function GetValue() ; keep this\n');

    const actions = provider.provideCodeActions(document, { start: { line: 0 } });

    assert.deepEqual(actions[0].edit.inserts, [
      { uri: document.uri, position: { line: 0, character: 'Int Function GetValue() ; keep this'.length }, newText: ' @nodiscard' },
    ]);
  });
});

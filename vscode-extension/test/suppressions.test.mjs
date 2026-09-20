import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import suppressions from '../out-test/src/suppressions.js';

const {
  addFileDisableComment,
  disableRuleInConfigYaml,
  fileDisableCovers,
  isIgnorableRule,
  ruleConfigKey,
} = suppressions;

describe('rule ids', () => {
  it('maps hyphenated diagnostic ids onto YAML keys', () => {
    assert.equal(ruleConfigKey('trailing-whitespace'), 'trailing_whitespace');
    assert.equal(ruleConfigKey('float_to_int'), 'float_to_int');
  });

  it('does not treat compiler-error as ignorable', () => {
    assert.equal(isIgnorableRule('trailing-whitespace'), true);
    assert.equal(isIgnorableRule('compiler-error'), false);
    assert.equal(isIgnorableRule(''), false);
  });
});

describe('addFileDisableComment', () => {
  it('inserts a file directive as the first line', () => {
    assert.equal(
      addFileDisableComment('ScriptName Example\n', 'trailing-whitespace'),
      '; @disable-file trailing-whitespace\nScriptName Example\n',
    );
  });

  it('uses the file\'s own newline style and handles an empty buffer', () => {
    assert.equal(
      addFileDisableComment('ScriptName Example\r\n', 'comma-spacing'),
      '; @disable-file comma-spacing\r\nScriptName Example\r\n',
    );
    assert.equal(addFileDisableComment('', 'comma-spacing'), '; @disable-file comma-spacing\n');
    assert.equal(addFileDisableComment('ScriptName Example', 'comma-spacing'), '; @disable-file comma-spacing\nScriptName Example');
  });

  it('merges into an existing named @disable-file and is a no-op when already covered', () => {
    const source = 'ScriptName Example ; @disable-file comma-spacing\n';
    assert.equal(
      addFileDisableComment(source, 'trailing-whitespace'),
      'ScriptName Example ; @disable-file comma-spacing, trailing-whitespace\n',
    );
    assert.equal(addFileDisableComment(source, 'COMMA-SPACING'), source);
    assert.equal(fileDisableCovers(source, 'comma-spacing'), true);
    assert.equal(fileDisableCovers(source, 'trailing-whitespace'), false);
  });

  it('leaves a bare @disable-file untouched and ignores lookalikes', () => {
    const bare = '; @disable-file\nScriptName Example\n';
    assert.equal(addFileDisableComment(bare, 'comma-spacing'), bare);
    assert.equal(fileDisableCovers(bare, 'comma-spacing'), true);
    const lookalike = 'ScriptName Example ; @disable-file-thing comma-spacing\n';
    assert.equal(
      addFileDisableComment(lookalike, 'comma-spacing'),
      '; @disable-file comma-spacing\nScriptName Example ; @disable-file-thing comma-spacing\n',
    );
    const lineDisable = 'Call(1,2) ; @disable comma-spacing\n';
    assert.equal(
      addFileDisableComment(lineDisable, 'comma-spacing'),
      '; @disable-file comma-spacing\nCall(1,2) ; @disable comma-spacing\n',
    );
  });

  it('does not treat a semicolon inside a string as a comment', () => {
    const source = 'String Message = "; @disable-file comma-spacing"\n';
    assert.equal(
      addFileDisableComment(source, 'comma-spacing'),
      '; @disable-file comma-spacing\nString Message = "; @disable-file comma-spacing"\n',
    );
    assert.equal(
      addFileDisableComment('String Message = "say \\"hi\\"; still code"\n', 'comma-spacing'),
      '; @disable-file comma-spacing\nString Message = "say \\"hi\\"; still code"\n',
    );
  });

  it('is a no-op for an empty rule id', () => {
    assert.equal(addFileDisableComment('ScriptName Example\n', ''), 'ScriptName Example\n');
  });
});

describe('disableRuleInConfigYaml', () => {
  it('creates a rules block for an empty or rules-less file', () => {
    assert.equal(disableRuleInConfigYaml('', 'trailing-whitespace'), 'rules:\n  trailing_whitespace: false\n');
    assert.equal(
      disableRuleInConfigYaml('semicolon: true\n', 'trailing-whitespace'),
      'semicolon: true\nrules:\n  trailing_whitespace: false\n',
    );
  });

  it('flips an existing true toggle and preserves comments', () => {
    const yaml = '# Each rule accepts true or false\nrules:\n  trailing_whitespace: true  # keep\n  comma_spacing: true\n';
    assert.equal(
      disableRuleInConfigYaml(yaml, 'trailing-whitespace'),
      '# Each rule accepts true or false\nrules:\n  trailing_whitespace: false  # keep\n  comma_spacing: true\n',
    );
  });

  it('is a no-op when the rule is already false', () => {
    const yaml = 'rules:\n  trailing_whitespace: false\n';
    assert.equal(disableRuleInConfigYaml(yaml, 'trailing-whitespace'), yaml);
  });

  it('inserts a missing key using the existing child indent', () => {
    const yaml = 'rules:\n    comma_spacing: true\nsemicolon: false\n';
    assert.equal(
      disableRuleInConfigYaml(yaml, 'trailing-whitespace'),
      'rules:\n    trailing_whitespace: false\n    comma_spacing: true\nsemicolon: false\n',
    );
  });

  it('skips comments and blanks inside the rules block', () => {
    const yaml = 'rules:\n  # keep\n\n  comma_spacing: true\n';
    assert.equal(
      disableRuleInConfigYaml(yaml, 'trailing-whitespace'),
      'rules:\n  trailing_whitespace: false\n  # keep\n\n  comma_spacing: true\n',
    );
  });

  it('does not match a longer key that shares a prefix', () => {
    const yaml = 'rules:\n  trailing_whitespace_extra: true\n';
    assert.equal(
      disableRuleInConfigYaml(yaml, 'trailing-whitespace'),
      'rules:\n  trailing_whitespace: false\n  trailing_whitespace_extra: true\n',
    );
  });

  it('updates a flow-style rules mapping', () => {
    assert.equal(
      disableRuleInConfigYaml('rules: { comma_spacing: true, trailing_whitespace: true }\n', 'trailing-whitespace'),
      'rules: { comma_spacing: true, trailing_whitespace: false }\n',
    );
    assert.equal(
      disableRuleInConfigYaml('rules: { comma_spacing: true }\n', 'trailing-whitespace'),
      'rules: { comma_spacing: true, trailing_whitespace: false }\n',
    );
    assert.equal(
      disableRuleInConfigYaml('rules: {}\n', 'trailing-whitespace'),
      'rules: { trailing_whitespace: false }\n',
    );
    assert.equal(
      disableRuleInConfigYaml('rules: { trailing_whitespace: true\n', 'trailing-whitespace'),
      'rules:\n  trailing_whitespace: false\n',
    );
  });

  it('preserves CRLF and quoted keys', () => {
    assert.equal(
      disableRuleInConfigYaml('rules:\r\n  "trailing_whitespace": true\r\n', 'trailing-whitespace'),
      'rules:\r\n  "trailing_whitespace": false\r\n',
    );
  });

  it('is a no-op for an empty rule id', () => {
    assert.equal(disableRuleInConfigYaml('rules:\n  comma_spacing: true\n', ''), 'rules:\n  comma_spacing: true\n');
  });
});

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import nodiscardHeader from '../out-test/src/nodiscardHeader.js';

const { isAlreadyNodiscard, isEligibleNodiscardHeader, nodiscardInsertion } = nodiscardHeader;

describe('isEligibleNodiscardHeader', () => {
  it('flags a function that returns a value', () => {
    assert.equal(isEligibleNodiscardHeader('Int Function GetValue()'), true);
  });

  it('flags a native function with no return value', () => {
    assert.equal(isEligibleNodiscardHeader('Function DoThing() Native'), true);
  });

  it('flags a native function that also returns a value', () => {
    assert.equal(isEligibleNodiscardHeader('Int Function GetValue() Native'), true);
  });

  it('does not flag a void, non-native function', () => {
    assert.equal(isEligibleNodiscardHeader('Function DoThing()'), false);
  });

  it('does not flag an Event declaration', () => {
    assert.equal(isEligibleNodiscardHeader('Event OnInit()'), false);
  });

  it('ignores the word Native inside a trailing comment', () => {
    assert.equal(isEligibleNodiscardHeader('Function DoThing() ; behaves like a Native call'), false);
  });

  it('does not flag an unrelated statement', () => {
    assert.equal(isEligibleNodiscardHeader('result = ComputeValue(1)'), false);
  });

  it('ignores a semicolon inside a string literal default value', () => {
    assert.equal(isEligibleNodiscardHeader('Function Log(String msg = "a; b") Native'), true);
  });

  it('ignores an escaped quote inside a string literal', () => {
    assert.equal(isEligibleNodiscardHeader('Function Log(String msg = "a\\"; b") Native'), true);
  });
});

describe('isAlreadyNodiscard', () => {
  it('is true when the header line itself carries the flag', () => {
    assert.equal(isAlreadyNodiscard(['Int Function GetValue() ; @nodiscard'], 0), true);
  });

  it('is true when the line above carries the flag', () => {
    assert.equal(isAlreadyNodiscard(['; @nodiscard', 'Int Function GetValue()'], 1), true);
  });

  it('is false for @nodiscardable, which is a different word', () => {
    assert.equal(isAlreadyNodiscard(['Int Function GetValue() ; @nodiscardable'], 0), false);
  });

  it('is false when neither line is flagged', () => {
    assert.equal(isAlreadyNodiscard(['Int Function GetValue()'], 0), false);
  });
});

describe('nodiscardInsertion', () => {
  it('starts a new trailing comment when the line has none', () => {
    assert.equal(nodiscardInsertion('Int Function GetValue()'), ' ; @nodiscard');
  });

  it('extends an existing trailing comment', () => {
    assert.equal(nodiscardInsertion('Int Function GetValue() ; keep this'), ' @nodiscard');
  });

  it('ignores a semicolon inside a string literal default value', () => {
    assert.equal(nodiscardInsertion('Int Function Log(String s = "a; b")'), ' ; @nodiscard');
  });

  it('finds the real trailing comment past an escaped quote in a string literal', () => {
    assert.equal(nodiscardInsertion('Int Function Log(String s = "a\\"b") ; keep this'), ' @nodiscard');
  });
});

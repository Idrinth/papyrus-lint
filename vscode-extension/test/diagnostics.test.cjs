const assert = require('node:assert/strict');
const { describe, it } = require('node:test');
const { normalizeDiagnostic, parseReport } = require('../out-test/src/diagnostics');

describe('parseReport', () => {
  const report = {
    files: [],
    scripts_checked: 0,
    files_with_diagnostics: 0,
    total_diagnostics: 0,
    files_fixed: null,
    success: true,
  };

  it('parses JSON emitted by PapyrusLinterCLI', () => {
    assert.deepEqual(parseReport(JSON.stringify(report)), report);
  });

  it('rejects output that is not JSON', () => {
    assert.equal(parseReport('PapyrusLinterCLI failed'), undefined);
  });
});

describe('normalizeDiagnostic', () => {
  it('converts one-based CLI positions and preserves rule metadata', () => {
    assert.deepEqual(
      normalizeDiagnostic({
        line: 4,
        column: 7,
        rule: 'forbidden-function',
        level: 'warning',
        message: 'Do not call this function.',
      }),
      {
        line: 3,
        column: 6,
        level: 'warning',
        message: 'Do not call this function.',
        rule: 'forbidden-function',
      },
    );
  });

  it('maps informational and absent levels', () => {
    const diagnostic = { line: 1, column: 1, rule: 'rule', message: 'message' };

    assert.equal(normalizeDiagnostic({ ...diagnostic, level: 'info' }).level, 'information');
    assert.equal(normalizeDiagnostic({ ...diagnostic, level: null }).level, 'error');
  });

  it('clamps invalid CLI positions to the start of the document', () => {
    const normalized = normalizeDiagnostic({
      line: 0,
      column: -3,
      rule: 'test-rule',
      level: 'error',
      message: 'Invalid position.',
    });

    assert.equal(normalized.line, 0);
    assert.equal(normalized.column, 0);
  });
});

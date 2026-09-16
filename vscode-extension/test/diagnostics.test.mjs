import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import diagnostics from '../out-test/src/diagnostics.js';

const { normalizeDiagnostic, parseReport } = diagnostics;

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
        doc_url: 'https://papyrus-lint.idrinth.de/#lint-forbidden-discouraged-function-usage',
      }),
      {
        line: 3,
        column: 6,
        level: 'warning',
        message: 'Do not call this function.',
        rule: 'forbidden-function',
        docUrl: 'https://papyrus-lint.idrinth.de/#lint-forbidden-discouraged-function-usage',
      },
    );
  });

  it('passes through a null doc_url for a rule with no known tag metadata', () => {
    const normalized = normalizeDiagnostic({
      line: 1,
      column: 1,
      rule: 'compiler-error',
      level: 'error',
      message: 'syntax error',
      doc_url: null,
    });

    assert.equal(normalized.docUrl, null);
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

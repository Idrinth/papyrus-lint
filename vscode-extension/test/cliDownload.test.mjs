import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import cliDownload from '../out-test/src/cliDownload.js';

const { ensureReleaseCli } = cliDownload;

describe('ensureReleaseCli', () => {
  it('rejects platforms for which the release has no CLI asset', async () => {
    await assert.rejects(
      ensureReleaseCli('/unused', '1.2.3', 'freebsd'),
      /does not publish a CLI for freebsd/,
    );
  });
});

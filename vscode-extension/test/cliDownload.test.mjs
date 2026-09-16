import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';
import { afterEach, describe, it } from 'node:test';

const require = createRequire(import.meta.url);
const cliDownloadModule = path.resolve('out-test/src/cliDownload.js');
const originalFetch = globalThis.fetch;
const temporaryDirectories = [];

function loadCliDownload(responses = []) {
  const requests = [];

  globalThis.fetch = async (url) => {
    requests.push(String(url));
    const next = responses.shift();
    if (next instanceof Error) {
      throw next;
    }

    const status = next && Object.hasOwn(next, 'status') ? next.status : 200;
    return {
      ok: status >= 200 && status < 300,
      status,
      body: Readable.toWeb(Readable.from(next?.body ?? 'downloaded CLI')),
    };
  };

  delete require.cache[cliDownloadModule];
  return { cliDownload: require(cliDownloadModule), requests };
}

async function temporaryDirectory() {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'papyrus-lint-vscode-'));
  temporaryDirectories.push(directory);
  return directory;
}

afterEach(async () => {
  globalThis.fetch = originalFetch;
  delete require.cache[cliDownloadModule];
  await Promise.all(temporaryDirectories.splice(0).map((directory) => fs.rm(directory, { recursive: true, force: true })));
});

describe('ensureReleaseCli', () => {
  it('rejects platforms for which the release has no CLI asset', async () => {
    const { cliDownload } = loadCliDownload();
    await assert.rejects(
      cliDownload.ensureReleaseCli('/unused', '1.2.3', 'freebsd'),
      /does not publish a CLI for freebsd/,
    );
  });

  for (const [platform, asset] of [
    ['win32', 'PapyrusLinterCLI-windows.exe'],
    ['darwin', 'PapyrusLinterCLI-macos'],
    ['linux', 'PapyrusLinterCLI-linux'],
  ]) {
    it(`downloads and installs the ${platform} release asset`, async () => {
      const storage = await temporaryDirectory();
      const { cliDownload, requests } = loadCliDownload([{ body: `${platform} executable` }]);

      const executable = await cliDownload.ensureReleaseCli(storage, '1.2.3', platform);

      assert.equal(executable, path.join(storage, 'v1.2.3', asset));
      assert.equal(await fs.readFile(executable, 'utf8'), `${platform} executable`);
      assert.deepEqual(requests, [
        `https://github.com/Idrinth/papyrus-lint/releases/download/v1.2.3/${asset}`,
      ]);
      assert.equal((await fs.stat(executable)).mode & 0o777, 0o700);
    });
  }

  it('reuses an executable already stored for the extension version', async () => {
    const storage = await temporaryDirectory();
    const executable = path.join(storage, 'v1.2.3', 'PapyrusLinterCLI-linux');
    await fs.mkdir(path.dirname(executable));
    await fs.writeFile(executable, 'cached', { mode: 0o700 });
    const { cliDownload, requests } = loadCliDownload();

    assert.equal(await cliDownload.ensureReleaseCli(storage, '1.2.3', 'linux'), executable);
    assert.deepEqual(requests, []);
  });

  it('reports HTTP and request failures and removes partial downloads', async () => {
    const storage = await temporaryDirectory();
    let loaded = loadCliDownload([{ status: 503 }]);
    await assert.rejects(
      loaded.cliDownload.ensureReleaseCli(storage, '3.0.0', 'linux'),
      /download returned HTTP 503/,
    );

    loaded = loadCliDownload([new Error('network unavailable')]);
    await assert.rejects(
      loaded.cliDownload.ensureReleaseCli(storage, '3.0.1', 'linux'),
      /network unavailable/,
    );

    const files = await fs.readdir(path.join(storage, 'v3.0.1'));
    assert.deepEqual(files, []);
  });

  it('reports an unknown status when the response has no HTTP status code', async () => {
    const storage = await temporaryDirectory();
    const { cliDownload } = loadCliDownload([{ status: 0 }]);

    await assert.rejects(
      cliDownload.ensureReleaseCli(storage, '3.0.2', 'linux'),
      /download returned HTTP unknown/,
    );
  });
});

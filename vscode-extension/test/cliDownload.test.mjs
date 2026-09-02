import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { promises as fs } from 'node:fs';
import Module from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';
import { afterEach, describe, it } from 'node:test';

const cliDownloadModule = path.resolve('out-test/src/cliDownload.js');
const originalLoad = Module._load;
const temporaryDirectories = [];

function loadCliDownload(responses = []) {
  const requests = [];

  Module._load = function (request, parent, isMain) {
    if (request === 'https') {
      return {
        get(url, callback) {
          const pending = new EventEmitter();
          requests.push(String(url));
          queueMicrotask(() => {
            const next = responses.shift();
            if (next instanceof Error) {
              pending.emit('error', next);
              return;
            }

            const response = Readable.from(next?.body ?? 'downloaded CLI');
            response.statusCode = next?.statusCode ?? 200;
            response.headers = next?.headers ?? {};
            response.resume = response.resume.bind(response);
            callback(response);
          });
          return pending;
        },
      };
    }
    return originalLoad.call(this, request, parent, isMain);
  };

  delete Module._cache[cliDownloadModule];
  return { cliDownload: Module._load(cliDownloadModule, null, false), requests };
}

async function temporaryDirectory() {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'papyrus-lint-vscode-'));
  temporaryDirectories.push(directory);
  return directory;
}

afterEach(async () => {
  Module._load = originalLoad;
  delete Module._cache[cliDownloadModule];
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

  it('follows relative redirects before installing the download', async () => {
    const storage = await temporaryDirectory();
    const { cliDownload, requests } = loadCliDownload([
      { statusCode: 302, headers: { location: '/release-asset' } },
      { body: 'redirected executable' },
    ]);

    const executable = await cliDownload.ensureReleaseCli(storage, '2.0.0', 'linux');

    assert.equal(await fs.readFile(executable, 'utf8'), 'redirected executable');
    assert.equal(requests[1], 'https://github.com/release-asset');
  });

  it('reports HTTP and request failures and removes partial downloads', async () => {
    const storage = await temporaryDirectory();
    let loaded = loadCliDownload([{ statusCode: 503 }]);
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

  it('rejects redirects without a usable destination', async () => {
    const storage = await temporaryDirectory();
    const { cliDownload } = loadCliDownload([{ statusCode: 302 }]);

    await assert.rejects(
      cliDownload.ensureReleaseCli(storage, '4.0.0', 'linux'),
      /too many or invalid redirects/,
    );
  });
});

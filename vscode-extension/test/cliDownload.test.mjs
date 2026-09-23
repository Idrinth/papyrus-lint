import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
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

function sha256(body) {
  return createHash('sha256').update(body).digest('hex');
}

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
  const cliDownload = require(cliDownloadModule);
  return { cliDownload, requests };
}

function trust(cliDownload, asset, body, extra = []) {
  cliDownload.CLI_SHA256[asset] = [sha256(body), ...extra.map(sha256)];
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
      const body = `${platform} executable`;
      const { cliDownload, requests } = loadCliDownload([{ body }]);
      trust(cliDownload, asset, body);

      const executable = await cliDownload.ensureReleaseCli(storage, '1.2.3', platform);

      assert.equal(executable, path.join(storage, 'v1.2.3', asset));
      assert.equal(await fs.readFile(executable, 'utf8'), body);
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
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'cached');

    assert.equal(await cliDownload.ensureReleaseCli(storage, '1.2.3', 'linux'), executable);
    assert.deepEqual(requests, []);
  });

  it('downloads a new CLI when the extension version changes and removes the previous one', async () => {
    const storage = await temporaryDirectory();
    const previous = path.join(storage, 'v1.2.3', 'PapyrusLinterCLI-linux');
    await fs.mkdir(path.dirname(previous), { recursive: true });
    await fs.writeFile(previous, 'old', { mode: 0o700 });
    const leftover = path.join(storage, 'notes.txt');
    await fs.writeFile(leftover, 'keep');
    const { cliDownload, requests } = loadCliDownload([{ body: 'new CLI' }]);
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'new CLI');

    const executable = await cliDownload.ensureReleaseCli(storage, '1.2.4', 'linux');

    assert.equal(executable, path.join(storage, 'v1.2.4', 'PapyrusLinterCLI-linux'));
    assert.equal(await fs.readFile(executable, 'utf8'), 'new CLI');
    assert.deepEqual(requests, [
      'https://github.com/Idrinth/papyrus-lint/releases/download/v1.2.4/PapyrusLinterCLI-linux',
    ]);
    await assert.rejects(fs.access(previous), /ENOENT/);
    assert.equal(await fs.readFile(leftover, 'utf8'), 'keep');
  });

  it('removes leftover previous-version CLIs even when the current version is already cached', async () => {
    const storage = await temporaryDirectory();
    const current = path.join(storage, 'v2.0.0', 'PapyrusLinterCLI-linux');
    const previous = path.join(storage, 'v1.9.0', 'PapyrusLinterCLI-linux');
    await fs.mkdir(path.dirname(current), { recursive: true });
    await fs.mkdir(path.dirname(previous), { recursive: true });
    await fs.writeFile(current, 'current', { mode: 0o700 });
    await fs.writeFile(previous, 'previous', { mode: 0o700 });
    const { cliDownload, requests } = loadCliDownload();
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'current');

    assert.equal(await cliDownload.ensureReleaseCli(storage, '2.0.0', 'linux'), current);
    assert.deepEqual(requests, []);
    await assert.rejects(fs.access(previous), /ENOENT/);
    assert.equal(await fs.readFile(current, 'utf8'), 'current');
  });

  it('still uses a trusted cached CLI when pruning old versions fails', async () => {
    const storage = await temporaryDirectory();
    const current = path.join(storage, 'v2.0.0', 'PapyrusLinterCLI-linux');
    await fs.mkdir(path.dirname(current), { recursive: true });
    await fs.writeFile(current, 'current', { mode: 0o700 });
    const { cliDownload } = loadCliDownload();
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'current');
    const originalReaddir = fs.readdir;
    fs.readdir = async (target, options) => {
      if (target === storage && options?.withFileTypes) {
        throw new Error('directory is temporarily unavailable');
      }
      return originalReaddir(target, options);
    };

    try {
      assert.equal(await cliDownload.ensureReleaseCli(storage, '2.0.0', 'linux'), current);
    } finally {
      fs.readdir = originalReaddir;
    }
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

  it('rejects a download whose SHA-256 does not match the baked digest', async () => {
    const storage = await temporaryDirectory();
    const { cliDownload } = loadCliDownload([{ body: 'tampered' }]);
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'expected');

    await assert.rejects(
      cliDownload.ensureReleaseCli(storage, '3.0.3', 'linux'),
      /SHA-256 mismatch/,
    );
    assert.deepEqual(await fs.readdir(path.join(storage, 'v3.0.3')), []);
  });

  it('redownloads a cached file whose SHA-256 does not match', async () => {
    const storage = await temporaryDirectory();
    const executable = path.join(storage, 'v1.2.3', 'PapyrusLinterCLI-linux');
    await fs.mkdir(path.dirname(executable), { recursive: true });
    await fs.writeFile(executable, 'tampered cache', { mode: 0o700 });
    const { cliDownload, requests } = loadCliDownload([{ body: 'fresh' }]);
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'fresh');

    assert.equal(await cliDownload.ensureReleaseCli(storage, '1.2.3', 'linux'), executable);
    assert.equal(await fs.readFile(executable, 'utf8'), 'fresh');
    assert.deepEqual(requests, [
      'https://github.com/Idrinth/papyrus-lint/releases/download/v1.2.3/PapyrusLinterCLI-linux',
    ]);
  });

  it('reports a missing baked hash', async () => {
    const { cliDownload } = loadCliDownload();
    delete cliDownload.CLI_SHA256['PapyrusLinterCLI-linux'];
    assert.throws(() => cliDownload.expectedSha256('PapyrusLinterCLI-linux'), /no baked SHA-256/);
  });

  it('treats the first baked digest as the official CLI hash', () => {
    const { cliDownload } = loadCliDownload();
    cliDownload.CLI_SHA256['PapyrusLinterCLI-linux'] = ['aaa', 'bbb'];
    assert.equal(cliDownload.expectedSha256('PapyrusLinterCLI-linux'), 'aaa');
    assert.deepEqual(cliDownload.acceptedSha256s('PapyrusLinterCLI-linux'), ['aaa', 'bbb']);
    assert.equal(cliDownload.isAcceptedSha256('PapyrusLinterCLI-linux', 'aaa'), true);
    assert.equal(cliDownload.isAcceptedSha256('PapyrusLinterCLI-linux', 'bbb'), true);
    assert.equal(cliDownload.isAcceptedSha256('PapyrusLinterCLI-linux', 'ccc'), false);
  });

  it('accepts a user-supplied executable whose hash matches the CLI or GUI digest', async () => {
    const storage = await temporaryDirectory();
    const cliBody = 'official CLI';
    const guiBody = 'desktop app';
    const executable = path.join(storage, 'PapyrusLinter');
    await fs.writeFile(executable, guiBody, { mode: 0o700 });
    const { cliDownload } = loadCliDownload();
    trust(cliDownload, 'PapyrusLinterCLI-linux', cliBody, [guiBody]);

    await cliDownload.verifyConfiguredExecutable(executable, 'linux');
  });

  it('rejects a user-supplied executable whose hash is not baked in', async () => {
    const storage = await temporaryDirectory();
    const executable = path.join(storage, 'stranger');
    await fs.writeFile(executable, 'not a release binary', { mode: 0o700 });
    const { cliDownload } = loadCliDownload();
    trust(cliDownload, 'PapyrusLinterCLI-linux', 'official CLI', ['desktop app']);

    await assert.rejects(
      cliDownload.verifyConfiguredExecutable(executable, 'linux'),
      /configured executable SHA-256 mismatch/,
    );
  });
});

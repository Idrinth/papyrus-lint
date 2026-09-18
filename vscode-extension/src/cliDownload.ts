import { createHash } from 'crypto';
import { constants, createReadStream, createWriteStream, promises as fs } from 'fs';
import * as path from 'path';
import { Readable } from 'stream';
import { pipeline } from 'stream/promises';
import { CLI_SHA256 } from './cliHashes';

export { CLI_SHA256 };

const RELEASE_BASE = 'https://github.com/Idrinth/papyrus-lint/releases/download';

export function assetName(platform: NodeJS.Platform): string {
  switch (platform) {
    case 'win32':
      return 'PapyrusLinterCLI-windows.exe';
    case 'darwin':
      return 'PapyrusLinterCLI-macos';
    case 'linux':
      return 'PapyrusLinterCLI-linux';
    default:
      throw new Error(`Papyrus Lint does not publish a CLI for ${platform}.`);
  }
}

function bakedDigests(asset: string): readonly string[] {
  const expected = CLI_SHA256[asset];
  if (!expected || expected.length === 0) {
    throw new Error(`no baked SHA-256 for ${asset}`);
  }
  return expected;
}

/** SHA-256 of the official standalone CLI asset for this release. */
export function expectedSha256(asset: string): string {
  return bakedDigests(asset)[0];
}

/** Every SHA-256 accepted for `asset`: the CLI, then any GUI alternatives. */
export function acceptedSha256s(asset: string): readonly string[] {
  return bakedDigests(asset);
}

export function isAcceptedSha256(asset: string, digest: string): boolean {
  return acceptedSha256s(asset).includes(digest);
}

export async function sha256File(filePath: string): Promise<string> {
  const hash = createHash('sha256');
  await pipeline(createReadStream(filePath), hash);
  return hash.digest('hex');
}

async function assertExpectedSha256(filePath: string, asset: string): Promise<void> {
  const actual = await sha256File(filePath);
  const expected = expectedSha256(asset);
  if (actual !== expected) {
    throw new Error(
      `${asset} SHA-256 mismatch (expected ${expected}, got ${actual}); refusing to use a manipulated file`,
    );
  }
}

/** Require a user-supplied executable to match this release's CLI or GUI digest. */
export async function verifyConfiguredExecutable(
  executable: string,
  platform: NodeJS.Platform = process.platform,
): Promise<void> {
  const asset = assetName(platform);
  const actual = await sha256File(executable);
  if (!isAcceptedSha256(asset, actual)) {
    throw new Error(
      `configured executable SHA-256 mismatch (got ${actual}); ` +
        `refusing to use a file that is not this release's PapyrusLinterCLI or PapyrusLinter`,
    );
  }
}

async function download(url: string, destination: string): Promise<void> {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(`download returned HTTP ${response.status || 'unknown'}`);
  }
  await pipeline(Readable.fromWeb(response.body), createWriteStream(destination, { mode: 0o700 }));
}

/** Removes CLIs cached for other extension versions, so an update doesn't keep serving an old binary. */
async function pruneOtherVersions(storageDirectory: string, version: string): Promise<void> {
  const keep = `v${version}`;
  try {
    const entries = await fs.readdir(storageDirectory, { withFileTypes: true });
    await Promise.all(
      entries
        .filter((entry) => entry.isDirectory() && entry.name.startsWith('v') && entry.name !== keep)
        .map((entry) => fs.rm(path.join(storageDirectory, entry.name), { recursive: true, force: true })),
    );
  } catch {
    // Best-effort: a leftover previous-version CLI is harmless next to the current one.
  }
}

async function isTrustedExecutable(executable: string, asset: string): Promise<boolean> {
  try {
    await fs.access(executable, constants.X_OK);
    await assertExpectedSha256(executable, asset);
    return true;
  } catch {
    return false;
  }
}

/** Returns this extension release's CLI, downloading it when this version isn't cached yet. */
export async function ensureReleaseCli(
  storageDirectory: string,
  version: string,
  platform: NodeJS.Platform = process.platform,
): Promise<string> {
  const asset = assetName(platform);
  const directory = path.join(storageDirectory, `v${version}`);
  const executable = path.join(directory, asset);
  if (await isTrustedExecutable(executable, asset)) {
    await pruneOtherVersions(storageDirectory, version);
    return executable;
  }

  await fs.mkdir(directory, { recursive: true });
  const temporary = `${executable}.${process.pid}.download`;
  try {
    await download(`${RELEASE_BASE}/v${version}/${asset}`, temporary);
    await assertExpectedSha256(temporary, asset);
    await fs.chmod(temporary, 0o700);
    await fs.rename(temporary, executable);
  } finally {
    await fs.rm(temporary, { force: true });
  }
  await pruneOtherVersions(storageDirectory, version);
  return executable;
}

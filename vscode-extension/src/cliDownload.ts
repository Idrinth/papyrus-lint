import { constants, createWriteStream, promises as fs } from 'fs';
import * as path from 'path';
import { Readable } from 'stream';
import { pipeline } from 'stream/promises';

const RELEASE_BASE = 'https://github.com/Idrinth/papyrus-lint/releases/download';

function assetName(platform: NodeJS.Platform): string {
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

async function download(url: string, destination: string): Promise<void> {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(`download returned HTTP ${response.status || 'unknown'}`);
  }
  await pipeline(Readable.fromWeb(response.body), createWriteStream(destination, { mode: 0o700 }));
}

/** Returns this extension release's CLI, downloading it once into extension storage. */
export async function ensureReleaseCli(
  storageDirectory: string,
  version: string,
  platform: NodeJS.Platform = process.platform,
): Promise<string> {
  const asset = assetName(platform);
  const directory = path.join(storageDirectory, `v${version}`);
  const executable = path.join(directory, asset);
  try {
    await fs.access(executable, constants.X_OK);
    return executable;
  } catch {
    // Missing (or not executable): replace it atomically below.
  }

  await fs.mkdir(directory, { recursive: true });
  const temporary = `${executable}.${process.pid}.download`;
  try {
    await download(`${RELEASE_BASE}/v${version}/${asset}`, temporary);
    await fs.chmod(temporary, 0o700);
    await fs.rename(temporary, executable);
  } finally {
    await fs.rm(temporary, { force: true });
  }
  return executable;
}

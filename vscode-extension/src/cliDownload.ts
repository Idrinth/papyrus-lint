import { constants, createWriteStream, promises as fs } from 'fs';
import { get } from 'https';
import * as path from 'path';

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

function download(url: URL, destination: string, redirects = 0): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = get(url, (response) => {
      if (response.statusCode && response.statusCode >= 300 && response.statusCode < 400) {
        const location = response.headers.location;
        response.resume();
        if (!location || redirects >= 5) {
          reject(new Error('too many or invalid redirects'));
          return;
        }
        void download(new URL(location, url), destination, redirects + 1).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`download returned HTTP ${response.statusCode ?? 'unknown'}`));
        return;
      }

      const output = createWriteStream(destination, { mode: 0o700 });
      response.pipe(output);
      output.on('finish', () => output.close(() => resolve()));
      output.on('error', reject);
      response.on('error', reject);
    });
    request.on('error', reject);
  });
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
    await download(new URL(`${RELEASE_BASE}/v${version}/${asset}`), temporary);
    await fs.chmod(temporary, 0o700);
    await fs.rename(temporary, executable);
  } finally {
    await fs.rm(temporary, { force: true });
  }
  return executable;
}

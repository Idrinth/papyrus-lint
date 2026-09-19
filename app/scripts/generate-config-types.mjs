#!/usr/bin/env node
// Thin launcher so `npm run generate:config-types` works on Windows
// release runners (`python`) and everywhere else (`python3`). The
// generator itself lives in `.github/scripts/generate_config_types.py`.

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const script = path.resolve(appDir, "../.github/scripts/generate_config_types.py");

let lastError;
for (const bin of ["python3", "python"]) {
  const result = spawnSync(bin, [script, ...process.argv.slice(2)], {
    stdio: "inherit",
  });
  if (result.error && result.error.code === "ENOENT") {
    lastError = result.error;
    continue;
  }
  process.exit(result.status ?? 1);
}

console.error(`Failed to find python3 or python to run ${script}`);
if (lastError) {
  console.error(lastError);
}
process.exit(1);

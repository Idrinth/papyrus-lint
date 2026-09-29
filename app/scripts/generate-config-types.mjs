#!/usr/bin/env node
// Generates app/src/config-types.ts from shared/rules/*.json,
// shared/configuration/papyrus-lint.default.yaml, and shared/configuration/lint-settings.yaml.
// Mirrors papyrus-lints/build.rs writing Rules / default_rules() and Config
// into $OUT_DIR.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function loadYamlFile(filePath) {
  const py = [
    "import json, sys",
    "try:",
    " import yaml",
    "except ImportError:",
    " import subprocess",
    " subprocess.check_call([sys.executable, '-m', 'pip', 'install', '--user', '-q', 'PyYAML==6.0.2'])",
    " import yaml",
    "json.dump(yaml.safe_load(open(sys.argv[1], encoding='utf-8')), sys.stdout)",
  ].join("\n");
  const result = spawnSync("python3", ["-c", py, filePath], { encoding: "utf8" });
  if (result.error) {
    throw new Error(`could not parse ${filePath}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`could not parse ${filePath}: ${(result.stderr || "").trim() || `exit ${result.status}`}`);
  }
  return JSON.parse(result.stdout);
}

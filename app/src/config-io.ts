import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_LINT_CONFIG, type LintConfig } from "./config-types";
// Looks for a papyrus-lint YAML config file in `dir`, falling back to the
// default configuration if none is found.
export async function loadLintConfig(dir: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config", { dir });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

// Persists `config` to `dir`'s papyrus-lint YAML config file so the
// formatting selected in the UI is remembered for next time.
export async function saveLintConfig(dir: string, config: LintConfig): Promise<void> {
  try {
    await invoke("save_lint_config", { dir, config });
  } catch (error) {
    console.error(error);
  }
}

// Reads and parses the config file at the exact `path` given, bypassing the
// project-directory discovery loadLintConfig does. Backs the Settings tab's
// "Configuration file" override.
export async function loadLintConfigFromPath(path: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config_from_path", { path });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

// Persists `config` to the exact file at `path`, creating it if it doesn't
// exist yet. The save-side counterpart of loadLintConfigFromPath, used
// while the Settings tab's "Configuration file" override is set.
export async function saveLintConfigToPath(path: string, config: LintConfig): Promise<void> {
  try {
    await invoke("save_lint_config_to_path", { path, config });
  } catch (error) {
    console.error(error);
  }
}

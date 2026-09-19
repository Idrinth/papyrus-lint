import { invoke } from "@tauri-apps/api/core";
import { type LintConfig } from "./config-types";
import { type ConfigPreset } from "./main-types";
// plus any user preset (see ConfigPreset) — for the config-picker dialog's
// inline preset list (see promptForConfigSelection). Returns an empty
// array if the lookup fails, which promptForConfigSelection treats the
// same as "nothing to offer" and hides that list entirely.
export async function loadConfigPresets(): Promise<ConfigPreset[]> {
  try {
    return (await invoke<ConfigPreset[]>("list_config_presets")) ?? [];
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Seeds `dir`'s papyrus-lint config file from the named preset (built-in
// or user). Only called right after promptForConfigSelection resolves with
// a "preset" choice, while `dir` is still known to have no config file of
// its own.
export async function applyConfigPreset(dir: string, preset: string): Promise<void> {
  try {
    await invoke("apply_config_preset", { dir, preset });
  } catch (error) {
    console.error(error);
  }
}

// Fetches `preset`'s lint rule/formatting settings only (built-in or
// user), via the backend's get_preset_lint_config command
// (papyrus_lint_config::preset_lint_config_default) — the
// config-only half of what applyConfigPreset seeds a brand new project's
// file with. Used by handleResetToPresetClick to overwrite the Settings
// tab's currently edited settings with a preset's own, in an existing
// project that already has a configuration.
export async function getPresetLintConfig(preset: string): Promise<LintConfig> {
  return invoke<LintConfig>("get_preset_lint_config", { preset });
}

// Renames the user preset `oldName` to `newName`, via the backend's
// rename_user_preset command (papyrus_lint_config::rename_user_preset).
export async function renameUserPreset(oldName: string, newName: string, overwrite: boolean): Promise<void> {
  await invoke("rename_user_preset", { oldName, newName, overwrite });
}

// Deletes the user preset `name`, via the backend's delete_user_preset
// command (papyrus_lint_config::delete_user_preset).
export async function deleteUserPreset(name: string): Promise<void> {
  await invoke("delete_user_preset", { name });
}

// Fetches the user preset `name`'s raw YAML content, via the backend's
// export_user_preset command (papyrus_lint_config::read_user_preset_yaml),
// for handleExportPresetClick to offer as a download.
export async function exportUserPreset(name: string): Promise<string> {
  return invoke<string>("export_user_preset", { name });
}

// The three built-in presets' own ids (see papyrus_lint_config::PRESET_NAMES),
// kept in sync by hand the same way FIXABLE_RULE_IDS is: everything
// loadConfigPresets returns that isn't one of these is a user preset, since
// config::save_user_preset/rename_user_preset always refuse a name matching
// one of these case-insensitively.
const BUILTIN_PRESET_IDS = new Set(["strict", "standard", "careful"]);

export function isCustomPreset(preset: ConfigPreset): boolean {
  return !BUILTIN_PRESET_IDS.has(preset.id.toLowerCase());
}

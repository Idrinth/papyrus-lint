import { invoke } from "@tauri-apps/api/core";
import { type ConfigPreset, type ConfigSelectionResult, switchTab } from "./main";
import { type LintConfig, applyLintConfigToUI, currentLintConfig, handleLintConfigChanged } from "./config";
import { type ProjectInfo } from "./project";
import { downloadTextFile } from "./download-text-file";

export let saveConfigAsPresetButtonEl: HTMLButtonElement | null;
export let resetToPresetSelectEl: HTMLSelectElement | null;
export let resetToPresetButtonEl: HTMLButtonElement | null;
export let configPickerEl: HTMLDialogElement | null;
export let configPickerDetectedEl: HTMLElement | null;
export let configPickerDetectedPathEl: HTMLElement | null;
export let configPickerNoneEl: HTMLElement | null;
export let configPickerPresetListEl: HTMLElement | null;
export let configPickerPathInputEl: HTMLInputElement | null;
export let configPickerUsePathButtonEl: HTMLButtonElement | null;
export let configPickerContinueEl: HTMLButtonElement | null;
export let presetManagementTabEl: HTMLButtonElement | null;
export let presetManagementListEl: HTMLElement | null;

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

// Prompts for a name and saves the Settings tab's currently edited lint
// configuration (currentLintConfig, kept in sync by handleLintConfigChanged)
// as a new user preset under it, via the backend's save_config_as_preset
// command (papyrus_lint_config::save_user_preset) — the same
// executable-adjacent "presets" directory loadConfigPresets/applyConfigPreset
// above already read from, so the saved preset is immediately selectable
// from the first-run picker (or the CLI's --preset <name>) afterward.
// Cancels silently if the prompt is left blank; if a preset (built-in or
// user) already exists under that name, asks to overwrite it and cancels
// silently if declined. Reports success/failure once the save itself is
// attempted, since unlike every other Settings tab field, this isn't an
// autosave the user can otherwise tell happened.
export async function handleSaveConfigAsPresetClick(): Promise<void> {
  const name = window.prompt("Save the current settings as a preset named:")?.trim();
  if (!name) {
    return;
  }

  const presets = await loadConfigPresets();
  const exists = presets.some((preset) => preset.id.toLowerCase() === name.toLowerCase());
  if (exists && !window.confirm(`A preset named "${name}" already exists. Overwrite it?`)) {
    return;
  }

  try {
    await invoke("save_config_as_preset", { config: currentLintConfig, name, overwrite: exists });
    await refreshPresetManagementTab();
    window.alert(`Saved preset "${name}".`);
  } catch (error) {
    console.error(error);
    window.alert(`Failed to save preset "${name}": ${error}`);
  }
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

// Rebuilds the Presets tab's management list from `presets` (see
// loadConfigPresets), showing only the user (non-built-in) ones — built-in
// presets can't be renamed, exported, or deleted. The tab itself (its
// button and panel) is only shown while at least one user preset exists;
// if it was the active tab and its last preset just got deleted, switches
// back to the Settings tab instead of leaving an empty panel showing.
export function renderPresetManagementTab(presets: ConfigPreset[]) {
  const customPresets = presets.filter(isCustomPreset);
  const hasCustomPresets = customPresets.length > 0;
  const wasActive = presetManagementTabEl?.classList.contains("tabs__tab--active") ?? false;
  if (presetManagementTabEl) {
    presetManagementTabEl.hidden = !hasCustomPresets;
  }
  if (!hasCustomPresets && wasActive) {
    switchTab("settings");
  }

  if (!presetManagementListEl) {
    return;
  }
  presetManagementListEl.innerHTML = "";
  for (const preset of customPresets) {
    const item = document.createElement("li");
    item.className = "preset-management__item";

    const label = document.createElement("span");
    label.className = "preset-management__label";
    label.textContent = preset.label;

    const actions = document.createElement("span");
    actions.className = "preset-management__actions";

    const renameButton = document.createElement("button");
    renameButton.type = "button";
    renameButton.className = "preset-management__button";
    renameButton.textContent = "Rename";
    renameButton.addEventListener("click", () => void handleRenamePresetClick(preset));

    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.className = "preset-management__button";
    exportButton.textContent = "Export";
    exportButton.addEventListener("click", () => void handleExportPresetClick(preset));

    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.className = "preset-management__button";
    deleteButton.textContent = "Delete";
    deleteButton.addEventListener("click", () => void handleDeletePresetClick(preset));

    actions.append(renameButton, exportButton, deleteButton);
    item.append(label, actions);
    presetManagementListEl.appendChild(item);
  }
}

// Reloads every configuration preset and re-renders the Presets tab, and
// the Settings tab's "Reset to preset" dropdown, from it. Called on
// startup and after any action (saving, renaming, or deleting a user
// preset) that could change which presets exist.
export async function refreshPresetManagementTab(): Promise<void> {
  const presets = await loadConfigPresets();
  renderPresetManagementTab(presets);
  populateResetPresetSelect(presets);
}

// Rebuilds the Settings tab's "Reset to preset" dropdown (see
// handleResetToPresetClick) from `presets` (built-in and user), kept in
// sync with the Presets tab via refreshPresetManagementTab above.
// Preserves the previously selected preset's id across a rebuild when it
// still exists, so saving/renaming/deleting an unrelated preset doesn't
// silently reset the dropdown back to its first option.
export function populateResetPresetSelect(presets: ConfigPreset[]) {
  if (!resetToPresetSelectEl) {
    return;
  }
  const previous = resetToPresetSelectEl.value;
  resetToPresetSelectEl.innerHTML = "";
  for (const preset of presets) {
    const option = document.createElement("option");
    option.value = preset.id;
    option.textContent = preset.label;
    resetToPresetSelectEl.appendChild(option);
  }
  if (presets.some((preset) => preset.id === previous)) {
    resetToPresetSelectEl.value = previous;
  }
}

// Overwrites the Settings tab's currently edited lint rule/formatting
// settings with the dropdown's selected preset's own, after confirming
// since this discards whatever's currently configured and can't be
// undone. Reuses applyLintConfigToUI/handleLintConfigChanged — the same
// "populate the form, then read it back and persist" path any manual edit
// already goes through — rather than a separate save call, so the reset
// is written wherever settings are already being saved (the current
// project directory, or an active "Configuration file" override).
export async function handleResetToPresetClick(): Promise<void> {
  const select = resetToPresetSelectEl;
  if (!select || !select.value) {
    return;
  }
  const label = select.options[select.selectedIndex]?.textContent ?? select.value;
  if (
    !window.confirm(
      `Reset all lint rule and formatting settings to the "${label}" preset? ` +
        "This overwrites your current settings and can't be undone.",
    )
  ) {
    return;
  }

  try {
    const config = await getPresetLintConfig(select.value);
    applyLintConfigToUI(config);
    handleLintConfigChanged();
  } catch (error) {
    console.error(error);
    window.alert(`Failed to reset settings to "${label}": ${error}`);
  }
}

// Prompts for `preset`'s new name, confirming an overwrite the same way
// handleSaveConfigAsPresetClick does if one is already in use, then renames
// it via renameUserPreset and refreshes the tab. Cancels silently if the
// prompt is left blank, unchanged (ignoring case), or the overwrite
// confirmation is declined.
export async function handleRenamePresetClick(preset: ConfigPreset): Promise<void> {
  const name = window.prompt(`Rename preset "${preset.label}" to:`, preset.label)?.trim();
  if (!name || name.toLowerCase() === preset.id.toLowerCase()) {
    return;
  }

  const presets = await loadConfigPresets();
  const exists = presets.some((other) => other.id.toLowerCase() === name.toLowerCase());
  if (exists && !window.confirm(`A preset named "${name}" already exists. Overwrite it?`)) {
    return;
  }

  try {
    await renameUserPreset(preset.id, name, exists);
    await refreshPresetManagementTab();
  } catch (error) {
    console.error(error);
    window.alert(`Failed to rename preset "${preset.label}": ${error}`);
  }
}

// Confirms, then deletes `preset` via deleteUserPreset and refreshes the
// tab.
export async function handleDeletePresetClick(preset: ConfigPreset): Promise<void> {
  if (!window.confirm(`Delete preset "${preset.label}"? This can't be undone.`)) {
    return;
  }

  try {
    await deleteUserPreset(preset.id);
    await refreshPresetManagementTab();
  } catch (error) {
    console.error(error);
    window.alert(`Failed to delete preset "${preset.label}": ${error}`);
  }
}

// Downloads `preset`'s raw YAML content (via exportUserPreset) as
// `<id>.yaml`, the same browser-download technique handleExportIssuesClick
// uses for the Lint results tab's own export button.
export async function handleExportPresetClick(preset: ConfigPreset): Promise<void> {
  try {
    const yaml = await exportUserPreset(preset.id);
    downloadTextFile(`${preset.id}.yaml`, yaml, "application/x-yaml");
  } catch (error) {
    console.error(error);
    window.alert(`Failed to export preset "${preset.label}": ${error}`);
  }
}

// Shows the "select this project's configuration" dialog useProjectDir
// opens for every not-yet-confirmed project directory (see
// confirmedProjectDirs), so a project's configuration is always picked
// with the project itself already known, rather than the Settings tab
// showing/editing whatever configuration happened to be loaded previously
// (or the engine's silent defaults) before the user has even said which
// project it applies to. Resolves to `{ kind: "detected" }` for "Continue"
// (or Escape/a backdrop click), which leaves useProjectDir's own
// auto-detection to do the right thing whether or not the project already
// has a configuration file; to `{ kind: "path", path }` once a non-blank
// path is confirmed via the "different file" input; or to
// `{ kind: "preset", preset }` once one of the inline preset options -
// shown only when the project has no configuration file yet, since
// initializing from a preset requires there to be none (see
// applyConfigPreset/papyrus_lint_config::initialize_default_config)
// - is clicked. Resolves immediately with `{ kind: "detected" }` if the
// dialog isn't present in the DOM (e.g. a minimal test fixture).
export async function promptForConfigSelection(projectInfo: ProjectInfo): Promise<ConfigSelectionResult> {
  if (!configPickerEl) {
    return { kind: "detected" };
  }

  const detectedPath = projectInfo.used_configuration_file;
  const presets = detectedPath ? [] : await loadConfigPresets();
  const dialog = configPickerEl;

  return new Promise((resolve) => {
    if (configPickerDetectedEl) {
      configPickerDetectedEl.hidden = !detectedPath;
    }
    if (configPickerDetectedPathEl) {
      configPickerDetectedPathEl.textContent = detectedPath ?? "";
    }
    if (configPickerNoneEl) {
      configPickerNoneEl.hidden = Boolean(detectedPath);
    }
    if (configPickerPathInputEl) {
      configPickerPathInputEl.value = "";
    }

    let settled = false;
    // configPickerContinueEl/configPickerUsePathButtonEl are static
    // elements reused across every call (unlike the preset options below,
    // rebuilt fresh each time), so their listeners must be explicitly torn
    // down here - otherwise an earlier, already-resolved call's handler
    // (still bound, since a run that resolved via a different path/Escape
    // never fired it to trigger its own removal) would keep piling up
    // across every project dropped in the session.
    const cleanup = () => {
      dialog.removeEventListener("close", handleClose);
      configPickerContinueEl?.removeEventListener("click", handleContinue);
      configPickerUsePathButtonEl?.removeEventListener("click", handleUsePath);
    };
    const finish = (result: ConfigSelectionResult) => {
      if (settled) {
        return;
      }
      settled = true;
      cleanup();
      if (dialog.hasAttribute("open")) {
        dialog.close();
      }
      resolve(result);
    };
    const handleClose = () => finish({ kind: "detected" });
    const handleContinue = () => finish({ kind: "detected" });
    const handleUsePath = () => {
      const path = configPickerPathInputEl?.value.trim();
      if (path) {
        finish({ kind: "path", path });
      }
    };

    configPickerContinueEl?.addEventListener("click", handleContinue);
    configPickerUsePathButtonEl?.addEventListener("click", handleUsePath);

    if (configPickerPresetListEl) {
      configPickerPresetListEl.hidden = presets.length === 0;
      configPickerPresetListEl.innerHTML = "";
      for (const preset of presets) {
        const option = document.createElement("button");
        option.type = "button";
        option.className = "config-picker__preset-option";
        const label = document.createElement("strong");
        label.className = "config-picker__preset-option-label";
        label.textContent = preset.label;
        const description = document.createElement("span");
        description.className = "config-picker__preset-option-description";
        description.textContent = preset.description;
        option.append(label, description);
        option.addEventListener("click", () => finish({ kind: "preset", preset: preset.id }));
        configPickerPresetListEl.appendChild(option);
      }
    }

    dialog.addEventListener("close", handleClose, { once: true });
    dialog.showModal();
  });
}


export function bindPresets() {
  saveConfigAsPresetButtonEl = document.querySelector("#save-config-as-preset");
  saveConfigAsPresetButtonEl?.addEventListener("click", () => void handleSaveConfigAsPresetClick());
  resetToPresetSelectEl = document.querySelector("#reset-to-preset-select");
  resetToPresetButtonEl = document.querySelector("#reset-to-preset");
  resetToPresetButtonEl?.addEventListener("click", () => void handleResetToPresetClick());
  presetManagementTabEl = document.querySelector("#tab-presets");
  presetManagementListEl = document.querySelector("#preset-management-list");

  configPickerEl = document.querySelector("#config-picker");
  configPickerDetectedEl = document.querySelector("#config-picker-detected");
  configPickerDetectedPathEl = document.querySelector("#config-picker-detected-path");
  configPickerNoneEl = document.querySelector("#config-picker-none");
  configPickerPresetListEl = document.querySelector("#config-picker-preset-list");
  configPickerPathInputEl = document.querySelector("#config-picker-path-input");
  configPickerUsePathButtonEl = document.querySelector("#config-picker-use-path");
  configPickerContinueEl = document.querySelector("#config-picker-continue");
  configPickerEl?.addEventListener("click", (event) => {
    if (event.target === configPickerEl) {
      configPickerEl?.close();
    }
  });
}

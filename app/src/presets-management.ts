import { invoke } from "@tauri-apps/api/core";
import { applyLintConfigToUI, handleLintConfigChanged } from "./config-ui";
import { currentLintConfig } from "./config-types";
import { downloadTextFile } from "./download-text-file";
import { type ConfigPreset } from "./main-types";
import { switchTab } from "./main-tabs";
import { deleteUserPreset, exportUserPreset, getPresetLintConfig, isCustomPreset, loadConfigPresets, renameUserPreset } from "./presets-api";
import { presetManagementListEl, presetManagementTabEl, resetToPresetSelectEl } from "./presets-state";
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
    // Game is a project target, not a formatting/rule preset. Resetting
    // careful/strict must not switch a Fallout 4 project back to Skyrim.
    applyLintConfigToUI({ ...config, game: currentLintConfig.game });
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

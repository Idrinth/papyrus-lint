// Config presets: picker, save/reset, Presets tab. Implementation lives in
// the sibling presets-*.ts modules; this file is the production façade so
// callers don't have to know which slice they need.

import { handleResetToPresetClick, handleSaveConfigAsPresetClick } from "./presets-management";
import * as presetsState from "./presets-state";

export { applyConfigPreset } from "./presets-api";
export { promptForConfigSelection } from "./presets-picker";
export { refreshPresetManagementTab } from "./presets-management";

export function bindPresets() {
  presetsState.saveConfigAsPresetButtonEl = document.querySelector("#save-config-as-preset");
  presetsState.saveConfigAsPresetButtonEl?.addEventListener("click", () => void handleSaveConfigAsPresetClick());
  presetsState.resetToPresetSelectEl = document.querySelector("#reset-to-preset-select");
  presetsState.resetToPresetButtonEl = document.querySelector("#reset-to-preset");
  presetsState.resetToPresetButtonEl?.addEventListener("click", () => void handleResetToPresetClick());
  presetsState.presetManagementTabEl = document.querySelector("#tab-presets");
  presetsState.presetManagementListEl = document.querySelector("#preset-management-list");

  presetsState.configPickerEl = document.querySelector("#config-picker");
  presetsState.configPickerDetectedEl = document.querySelector("#config-picker-detected");
  presetsState.configPickerDetectedPathEl = document.querySelector("#config-picker-detected-path");
  presetsState.configPickerNoneEl = document.querySelector("#config-picker-none");
  presetsState.configPickerPresetListEl = document.querySelector("#config-picker-preset-list");
  presetsState.configPickerPathInputEl = document.querySelector("#config-picker-path-input");
  presetsState.configPickerUsePathButtonEl = document.querySelector("#config-picker-use-path");
  presetsState.configPickerContinueEl = document.querySelector("#config-picker-continue");
  presetsState.configPickerEl?.addEventListener("click", (event) => {
    if (event.target === presetsState.configPickerEl) {
      presetsState.configPickerEl?.close();
    }
  });
}

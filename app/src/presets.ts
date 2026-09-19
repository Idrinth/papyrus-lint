// Config presets: picker, save/reset, Presets tab. Implementation lives in
// the sibling presets-*.ts modules; this file is the production façade so
// callers don't have to know which slice they need.

import { handleResetToPresetClick, handleSaveConfigAsPresetClick } from "./presets-management";
import {
  bindPresetsDom,
  configPickerEl,
  resetToPresetButtonEl,
  saveConfigAsPresetButtonEl,
} from "./presets-state";

export { applyConfigPreset } from "./presets-api";
export { promptForConfigSelection } from "./presets-picker";
export { refreshPresetManagementTab } from "./presets-management";

export function bindPresets() {
  bindPresetsDom();
  saveConfigAsPresetButtonEl?.addEventListener("click", () => void handleSaveConfigAsPresetClick());
  resetToPresetButtonEl?.addEventListener("click", () => void handleResetToPresetClick());
  configPickerEl?.addEventListener("click", (event) => {
    if (event.target === configPickerEl) {
      configPickerEl?.close();
    }
  });
}

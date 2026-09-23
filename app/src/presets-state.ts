export let saveConfigAsPresetButtonEl: HTMLButtonElement | null = null;
export let resetToPresetSelectEl: HTMLSelectElement | null = null;
export let resetToPresetButtonEl: HTMLButtonElement | null = null;
export let configPickerEl: HTMLDialogElement | null = null;
export let configPickerDetectedEl: HTMLElement | null = null;
export let configPickerDetectedPathEl: HTMLElement | null = null;
export let configPickerNoneEl: HTMLElement | null = null;
export let configPickerGameEl: HTMLElement | null = null;
export let configPickerGameSelectEl: HTMLSelectElement | null = null;
export let configPickerPresetListEl: HTMLElement | null = null;
export let configPickerPathInputEl: HTMLInputElement | null = null;
export let configPickerUsePathButtonEl: HTMLButtonElement | null = null;
export let configPickerContinueEl: HTMLButtonElement | null = null;
export let presetManagementTabEl: HTMLButtonElement | null = null;
export let presetManagementListEl: HTMLElement | null = null;

export function bindPresetsDom() {
  saveConfigAsPresetButtonEl = document.querySelector("#save-config-as-preset");
  resetToPresetSelectEl = document.querySelector("#reset-to-preset-select");
  resetToPresetButtonEl = document.querySelector("#reset-to-preset");
  presetManagementTabEl = document.querySelector("#tab-presets");
  presetManagementListEl = document.querySelector("#preset-management-list");
  configPickerEl = document.querySelector("#config-picker");
  configPickerDetectedEl = document.querySelector("#config-picker-detected");
  configPickerDetectedPathEl = document.querySelector("#config-picker-detected-path");
  configPickerNoneEl = document.querySelector("#config-picker-none");
  configPickerGameEl = document.querySelector("#config-picker-game");
  configPickerGameSelectEl = document.querySelector("#config-picker-game-select");
  configPickerPresetListEl = document.querySelector("#config-picker-preset-list");
  configPickerPathInputEl = document.querySelector("#config-picker-path-input");
  configPickerUsePathButtonEl = document.querySelector("#config-picker-use-path");
  configPickerContinueEl = document.querySelector("#config-picker-continue");
}

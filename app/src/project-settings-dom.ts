export let configPathOverrideEl: HTMLInputElement | null = null;
export let compilerPathEl: HTMLInputElement | null = null;
export let compileCheckEl: HTMLInputElement | null = null;
export let scriptRootsEl: HTMLTextAreaElement | null = null;
export let lookupScriptRootsEl: HTMLTextAreaElement | null = null;
export let detectedScriptRootsEl: HTMLOutputElement | null = null;
export let usedConfigurationFileEl: HTMLOutputElement | null = null;
export let settingsFieldsetEl: HTMLFieldSetElement | null = null;
export let settingsLockedNoticeEl: HTMLElement | null = null;

// Reads the Settings tab's "Configuration file" override input, trimmed. An
// empty string means no override is set, so the lint config is auto-detected
// from the current project directory as usual.
export function configPathOverride(): string {
  return configPathOverrideEl?.value.trim() ?? "";
}

export function bindProjectSettingsDom() {
  configPathOverrideEl = document.querySelector("#config-path-override");
  compilerPathEl = document.querySelector("#compiler-path");
  compileCheckEl = document.querySelector("#compile-check");
  scriptRootsEl = document.querySelector("#script-roots");
  lookupScriptRootsEl = document.querySelector("#lookup-script-roots");
  detectedScriptRootsEl = document.querySelector("#detected-script-roots");
  usedConfigurationFileEl = document.querySelector("#used-configuration-file");
  settingsFieldsetEl = document.querySelector("#settings-fieldset");
  settingsLockedNoticeEl = document.querySelector("#settings-locked-notice");
}

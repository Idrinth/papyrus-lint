// Settings controls for every Config field except `rules`. The markup is
// built from LINT_SETTINGS (configuration/lint-settings.json) so index.html
// only keeps the empty mounts.

import { type Game, type LintConfig, type LintSetting, LINT_SETTINGS } from "./config-types";
import { isSelectableGame } from "./main-types";

type SettingValue = string | number | boolean;
type SettingRecord = Record<string, SettingValue>;

function settingByKey(key: string): LintSetting | undefined {
  return LINT_SETTINGS.find((setting) => setting.key === key);
}

function control(id: string): HTMLInputElement | HTMLSelectElement | null {
  return document.querySelector(`#${id}`);
}

function selectedConfigValue(setting: LintSetting, uiValue: string): string {
  return setting.options?.find((option) => option.value === uiValue)?.config ?? uiValue;
}

function uiValueFor(setting: LintSetting, configValue: SettingValue): string {
  if (setting.widget === "bool-select") {
    return configValue ? (setting.trueValue ?? "true") : (setting.falseValue ?? "false");
  }
  if (setting.widget === "mapped-select") {
    return (
      setting.options?.find((option) => option.config === configValue)?.value ??
      String(configValue)
    );
  }
  return String(configValue);
}

function fillSelect(select: HTMLSelectElement, setting: LintSetting, selected?: string) {
  const options = setting.options ?? [];
  const preferred = selected ?? select.value;
  const match = options.some((option) => option.value === preferred)
    ? preferred
    : uiValueFor(setting, setting.defaultValue);
  select.replaceChildren();
  for (const option of options) {
    const el = document.createElement("option");
    el.value = option.value;
    el.textContent = option.label;
    if (option.value === match) {
      el.selected = true;
    }
    select.append(el);
  }
}

function renderSelect(setting: LintSetting): HTMLSelectElement {
  const select = document.createElement("select");
  select.id = setting.id;
  if (setting.ariaLabel) {
    select.setAttribute("aria-label", setting.ariaLabel);
  }
  fillSelect(select, setting, uiValueFor(setting, setting.defaultValue));
  return select;
}

function renderNumber(setting: LintSetting): HTMLInputElement {
  const input = document.createElement("input");
  input.id = setting.id;
  input.type = "number";
  if (setting.min != null) {
    input.min = String(setting.min);
  }
  if (setting.max != null) {
    input.max = String(setting.max);
  }
  if (setting.step != null) {
    input.step = String(setting.step);
  }
  input.value = String(setting.defaultValue);
  if (setting.disabledByDefault) {
    input.disabled = true;
  }
  return input;
}

function renderCheckbox(setting: LintSetting): HTMLInputElement {
  const input = document.createElement("input");
  input.id = setting.id;
  input.type = "checkbox";
  input.checked = setting.defaultValue === true;
  return input;
}

function renderControl(setting: LintSetting): HTMLElement {
  switch (setting.widget) {
    case "game":
    case "select":
    case "bool-select":
    case "mapped-select":
      return renderSelect(setting);
    case "number":
      return renderNumber(setting);
    case "checkbox":
      return renderCheckbox(setting);
    default:
      throw new Error(`unknown lint setting widget ${setting.widget}`);
  }
}

function renderLabel(setting: LintSetting, text: string): HTMLLabelElement {
  const label = document.createElement("label");
  label.htmlFor = setting.id;
  label.textContent = text;
  if (setting.title) {
    label.title = setting.title;
  }
  return label;
}

function renderCheckboxLabel(setting: LintSetting): HTMLLabelElement {
  const label = document.createElement("label");
  if (setting.title) {
    label.title = setting.title;
  }
  label.append(renderCheckbox(setting), document.createTextNode(` ${setting.checkboxLabel ?? ""}`));
  return label;
}

function renderGrouped(setting: LintSetting): HTMLElement {
  if (setting.widget === "checkbox") {
    return renderCheckboxLabel(setting);
  }
  if (setting.row) {
    const row = document.createElement("div");
    row.className = "settings-group__row";
    row.append(renderLabel(setting, setting.label ?? ""), renderControl(setting));
    return row;
  }
  return renderControl(setting);
}

function renderGroup(members: readonly LintSetting[]): HTMLElement {
  const first = members[0];
  if (first.groupKind === "indentation") {
    const wrap = document.createElement("div");
    wrap.className = "indentation-setting";
    wrap.append(document.createTextNode(`${first.groupPrefix ?? "Indentation"} `));
    for (const member of members) {
      if (member.widget === "number") {
        wrap.append(renderLabel(member, member.label ?? ""), renderNumber(member));
      } else {
        wrap.append(renderControl(member));
      }
    }
    return wrap;
  }
  const fieldset = document.createElement("fieldset");
  fieldset.className = "settings-group";
  const legend = document.createElement("legend");
  legend.textContent = first.groupLegend ?? "";
  fieldset.append(legend);
  for (const member of members) {
    fieldset.append(renderGrouped(member));
  }
  return fieldset;
}

function renderLoose(setting: LintSetting): DocumentFragment {
  const fragment = document.createDocumentFragment();
  if (setting.widget === "checkbox") {
    fragment.append(renderCheckboxLabel(setting));
    return fragment;
  }
  if (setting.label) {
    fragment.append(renderLabel(setting, setting.label));
  }
  fragment.append(renderControl(setting));
  return fragment;
}

function renderInto(parent: Element, settings: readonly LintSetting[]) {
  let index = 0;
  while (index < settings.length) {
    const setting = settings[index];
    if (!setting.group) {
      parent.append(renderLoose(setting));
      index += 1;
      continue;
    }
    const group = setting.group;
    const members: LintSetting[] = [];
    while (index < settings.length && settings[index].group === group) {
      members.push(settings[index]);
      index += 1;
    }
    parent.append(renderGroup(members));
  }
}

export function mountLintConfigControls() {
  const mounts = new Set(LINT_SETTINGS.map((setting) => setting.mount));
  for (const mountId of mounts) {
    const mount = document.querySelector(`#${mountId}`);
    if (!mount) {
      continue;
    }
    mount.replaceChildren();
    renderInto(
      mount,
      LINT_SETTINGS.filter((setting) => setting.mount === mountId),
    );
  }
  for (const setting of LINT_SETTINGS) {
    if (!setting.fillSelects) {
      continue;
    }
    for (const id of setting.fillSelects) {
      const select = document.querySelector<HTMLSelectElement>(`#${id}`);
      if (select) {
        fillSelect(select, setting);
      }
    }
  }
  syncLintSettingDependents();
}

function reflectGameControl(select: HTMLSelectElement, game: string) {
  for (const option of Array.from(select.querySelectorAll("option[data-unlisted-game]"))) {
    option.remove();
  }
  if (!Array.from(select.options).some((option) => option.value === game)) {
    const option = document.createElement("option");
    option.value = game;
    option.textContent = game;
    option.setAttribute("data-unlisted-game", "");
    select.append(option);
  }
  select.value = game;
}

function gameFromSelect(select: HTMLSelectElement | null, current: Game): Game {
  const selected = select?.value;
  if (isSelectableGame(selected)) {
    return selected;
  }
  const unlisted = select?.selectedOptions[0]?.hasAttribute("data-unlisted-game") === true;
  if (unlisted && selected) {
    return selected as Game;
  }
  return current;
}

function readNumber(setting: LintSetting, input: HTMLInputElement | null, soFar: SettingRecord): number {
  let raw = input?.valueAsNumber;
  if (raw == null || !Number.isFinite(raw) || (setting.treatZeroAsEmpty && raw === 0)) {
    raw = Number(setting.defaultValue);
  }
  if (setting.clampMin != null) {
    raw = Math.max(setting.clampMin, raw);
  }
  if (setting.clampMax != null) {
    raw = Math.min(setting.clampMax, raw);
  }
  if (setting.clampMinFrom) {
    const other = Number(soFar[setting.clampMinFrom]);
    if (Number.isFinite(other)) {
      raw = Math.max(other, raw);
    }
  }
  return raw;
}

function readOne(
  setting: LintSetting,
  el: HTMLInputElement | HTMLSelectElement | null,
  soFar: SettingRecord,
  currentGame: Game,
): SettingValue {
  switch (setting.widget) {
    case "game":
      return gameFromSelect(el as HTMLSelectElement | null, currentGame);
    case "checkbox":
      return el ? (el as HTMLInputElement).checked : setting.defaultValue === true;
    case "bool-select":
      if (!el) {
        return setting.defaultValue === true;
      }
      return el.value === setting.trueValue;
    case "mapped-select":
      if (!el) {
        return String(setting.defaultValue);
      }
      return selectedConfigValue(setting, el.value);
    case "select":
      return el ? el.value : String(setting.defaultValue);
    case "number":
      return readNumber(setting, el as HTMLInputElement | null, soFar);
    default:
      return setting.defaultValue;
  }
}

export function readLintSettingsFromUI(currentGame: Game): Omit<LintConfig, "rules"> {
  const result: SettingRecord = {};
  for (const setting of LINT_SETTINGS) {
    result[setting.key] = readOne(setting, control(setting.id), result, currentGame);
  }
  return result as Omit<LintConfig, "rules">;
}

export function applyLintSettingsToUI(config: LintConfig) {
  const values = config as unknown as SettingRecord;
  for (const setting of LINT_SETTINGS) {
    const el = control(setting.id);
    if (!el) {
      continue;
    }
    if (setting.widget === "game") {
      reflectGameControl(el as HTMLSelectElement, String(values[setting.key]));
      continue;
    }
    if (setting.widget === "checkbox") {
      (el as HTMLInputElement).checked = values[setting.key] === true;
      continue;
    }
    if (setting.widget === "number") {
      el.value = String(values[setting.key]);
      continue;
    }
    el.value = uiValueFor(setting, values[setting.key]);
  }
  syncLintSettingDependents();
}

export function syncLintSettingDependents() {
  for (const setting of LINT_SETTINGS) {
    if (!setting.enablesKey) {
      continue;
    }
    const source = document.querySelector<HTMLSelectElement>(`#${setting.id}`);
    const targetId = settingByKey(setting.enablesKey)?.id;
    const target = targetId ? document.querySelector<HTMLInputElement>(`#${targetId}`) : null;
    if (!source || !target) {
      continue;
    }
    target.disabled = selectedConfigValue(setting, source.value) !== setting.enablesWhen;
  }
}

export function lintSettingElements(): Array<HTMLInputElement | HTMLSelectElement> {
  return LINT_SETTINGS.flatMap((setting) => {
    const el = control(setting.id);
    return el ? [el] : [];
  });
}

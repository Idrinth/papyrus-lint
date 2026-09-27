import { markLintResultsStale } from "./drop";
import { loadLintConfig, loadLintConfigFromPath, saveLintConfig, saveLintConfigToPath } from "./config-io";
import { type Game, type LintConfig, type LintRules, DEFAULT_RULES, RULE_KEYS, RULE_SETTINGS, currentLintConfig, setCurrentLintConfig } from "./config-types";
import { isSelectableGame } from "./main-types";
import { applyLintSettingsToUI, lintSettingElements, mountLintConfigControls, readLintSettingsFromUI, syncLintSettingDependents } from "./lint-config-controls";
import { configPathOverride } from "./project-settings-dom";
import { currentProjectDir } from "./project-state";

let ruleEls: Partial<Record<keyof LintRules, HTMLInputElement>> = {};

// Fills `#lint-rules` from RULE_SETTINGS (generated with the rest of
// config-types.ts). index.html and the test fixture only keep the empty
// fieldset, so a new shared/rules entry shows up here without another edit.
function mountRuleControls() {
  const fieldset = document.querySelector("#lint-rules");
  if (!fieldset) {
    return;
  }
  for (const label of Array.from(fieldset.querySelectorAll("label"))) {
    label.remove();
  }
  for (const rule of RULE_SETTINGS) {
    const label = document.createElement("label");
    label.title = rule.description;
    const input = document.createElement("input");
    input.type = "checkbox";
    input.id = `rule-${rule.key}`;
    input.checked = DEFAULT_RULES[rule.key];
    label.append(input, document.createTextNode(` ${rule.name}`));
    fieldset.append(label);
  }
}

// Reflects `config` onto the formatting controls without firing their
// `change` listeners (assigning `.value` does not dispatch `change`).
export function applyLintConfigToUI(config: LintConfig) {
  applyLintSettingsToUI(config);
  for (const key of RULE_KEYS) {
    const el = ruleEls[key];
    if (el) {
      el.checked = config.rules[key];
    }
  }
}

function gameSelect(): HTMLSelectElement | null {
  return document.querySelector("#game-select");
}

// Sets the Settings tab's game control and persists it the same way a manual
// change would. Used by the first-run picker after a preset (or a continue
// that names a game) has been chosen for a project that had no config yet.
export function selectGame(game: Game): Promise<void> {
  const gameEl = gameSelect();
  if (!gameEl || !isSelectableGame(game)) {
    return Promise.resolve();
  }
  gameEl.value = game;
  return handleLintConfigChanged();
}

export function lintConfigFromUI(): LintConfig {
  const rules = { ...DEFAULT_RULES };
  for (const key of RULE_KEYS) {
    rules[key] = ruleEls[key]?.checked ?? DEFAULT_RULES[key];
  }
  return {
    ...readLintSettingsFromUI(currentLintConfig.game),
    rules,
  };
}

// Called whenever a formatting control changes: updates the in-memory
// config and, if a project directory is known, persists it to disk.
export function handleLintConfigChanged(): Promise<void> {
  setCurrentLintConfig(lintConfigFromUI());
  markLintResultsStale();
  const override = configPathOverride();
  if (override) {
    return saveLintConfigToPath(override, currentLintConfig);
  }
  if (currentProjectDir) {
    return saveLintConfig(currentProjectDir, currentLintConfig);
  }
  return Promise.resolve();
}

// Loads `dir`'s lint configuration (or the `overridePath` file instead, if
// set) into currentLintConfig and reflects it onto the Settings tab. The
// project-settings.ts counterpart to handleLintConfigChanged: called by useProjectDir
// when a project is (re)loaded, rather than in response to editing a
// formatting control.
export async function loadAndApplyLintConfig(dir: string, overridePath: string): Promise<void> {
  setCurrentLintConfig(overridePath ? await loadLintConfigFromPath(overridePath) : await loadLintConfig(dir));
  applyLintConfigToUI(currentLintConfig);
}

export function bindConfigSettings() {
  mountRuleControls();
  mountLintConfigControls();
  ruleEls = Object.fromEntries(
    RULE_KEYS.map((key) => [key, document.querySelector<HTMLInputElement>(`#rule-${key}`)]),
  ) as Partial<Record<keyof LintRules, HTMLInputElement>>;

  for (const el of lintSettingElements()) {
    el.addEventListener("change", () => {
      syncLintSettingDependents();
      void handleLintConfigChanged();
    });
  }
  for (const key of RULE_KEYS) {
    ruleEls[key]?.addEventListener("change", () => {
      void handleLintConfigChanged();
    });
  }
}

// Mirrors the id → Rules-field map `generate-config-types.mjs` bakes into
// RULE_SETTINGS. A hyphenated diagnostic id that isn't one of those rules
// (a compiler diagnostic, for example) has no Settings checkbox.
export function configKeyForRuleId(ruleId: string): keyof LintRules | undefined {
  return RULE_SETTINGS.find((rule) => rule.id === ruleId)?.key;
}

// Turns off every configurable rule in `ruleIds` in the in-memory lint
// config, reflects that onto the Settings tab, and persists it the same way
// unchecking those rules by hand would. Returns false when none of the ids
// map to a still-enabled rule, so the caller can no-op instead of re-linting.
export function disableRulesInLintConfig(ruleIds: string[]): boolean {
  const keys = Array.from(
    new Set(ruleIds.map(configKeyForRuleId).filter((key): key is keyof LintRules => key !== undefined)),
  );
  if (keys.length === 0) {
    return false;
  }
  const next = lintConfigFromUI();
  let changed = false;
  for (const key of keys) {
    if (next.rules[key]) {
      next.rules[key] = false;
      changed = true;
    }
  }
  if (!changed) {
    return false;
  }
  applyLintConfigToUI(next);
  handleLintConfigChanged();
  return true;
}

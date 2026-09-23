import { markLintResultsStale } from "./drop";
import { loadLintConfig, loadLintConfigFromPath, saveLintConfig, saveLintConfigToPath } from "./config-io";
import { type Game, type IdentifierCasingStyle, type LintConfig, type LintRules, type MagicNumbersMode, type NamedArgumentsStyle, type TypeCasingStyle, DEFAULT_RULES, RULE_KEYS, currentLintConfig, setCurrentLintConfig } from "./config-types";
import { isSelectableGame } from "./main-types";
import { configPathOverride } from "./project-settings-dom";
import { currentProjectDir } from "./project-state";
let gameEl: HTMLSelectElement | null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let typeCasingStyleEl: HTMLSelectElement | null;
let identifierCasingStyleEl: HTMLSelectElement | null;
let namedArgumentsStyleEl: HTMLSelectElement | null;
let magicNumbersModeEl: HTMLSelectElement | null;
let semicolonStyleEl: HTMLSelectElement | null;
let cyclomaticComplexityWarningEl: HTMLInputElement | null;
let cyclomaticComplexityErrorEl: HTMLInputElement | null;
let minWaitIntervalEl: HTMLInputElement | null;
let failOnWarningEl: HTMLInputElement | null;
let failOnInfoEl: HTMLInputElement | null;
let boolLikeIntEl: HTMLInputElement | null;
let assumeAutoPropertiesFilledEl: HTMLInputElement | null;
let ruleEls: Partial<Record<keyof LintRules, HTMLInputElement>> = {};

// Reflects `config` onto the formatting controls without firing their
// `change` listeners (assigning `.value` does not dispatch `change`).
export function applyLintConfigToUI(config: LintConfig) {
  reflectGameControl(config.game);
  if (semicolonStyleEl) {
    semicolonStyleEl.value = config.semicolon ? "require" : "forbid";
  }
  if (indentationStyleEl) {
    indentationStyleEl.value = config.indentation === "space" ? "spaces" : "tabs";
  }
  if (indentationWidthEl) {
    indentationWidthEl.value = String(config.indentation_width);
    indentationWidthEl.disabled = config.indentation !== "space";
  }
  if (cyclomaticComplexityWarningEl) {
    cyclomaticComplexityWarningEl.value = String(config.cyclomatic_complexity_warning);
  }
  if (cyclomaticComplexityErrorEl) {
    cyclomaticComplexityErrorEl.value = String(config.cyclomatic_complexity_error);
  }
  if (minWaitIntervalEl) {
    minWaitIntervalEl.value = String(config.min_wait_interval);
  }
  if (typeCasingStyleEl) {
    typeCasingStyleEl.value = config.type_casing;
  }
  if (identifierCasingStyleEl) {
    identifierCasingStyleEl.value = config.identifier_casing;
  }
  if (namedArgumentsStyleEl) {
    namedArgumentsStyleEl.value = config.named_arguments;
  }
  if (magicNumbersModeEl) {
    magicNumbersModeEl.value = config.magic_numbers;
  }
  if (failOnWarningEl) {
    failOnWarningEl.checked = config.fail_on_warning;
  }
  if (failOnInfoEl) {
    failOnInfoEl.checked = config.fail_on_info;
  }
  if (boolLikeIntEl) {
    boolLikeIntEl.checked = config.bool_like_int;
  }
  if (assumeAutoPropertiesFilledEl) {
    assumeAutoPropertiesFilledEl.checked = config.assume_auto_properties_filled;
  }
  for (const key of RULE_KEYS) {
    const el = ruleEls[key];
    if (el) {
      el.checked = config.rules[key];
    }
  }
}

// Reads the formatting controls' current values into a LintConfig.
function gameFromControls(): Game {
  const selected = gameEl?.value;
  if (isSelectableGame(selected)) {
    return selected;
  }
  // A loaded game the picker does not offer (CLI-only `starfield`) is shown
  // as an extra option. Honor that option when it is the current selection
  // so a later edit does not silently rewrite the key to Skyrim, and so
  // switching back to it still round-trips.
  const unlisted = gameEl?.selectedOptions[0]?.hasAttribute("data-unlisted-game") === true;
  if (unlisted && selected) {
    return selected as Game;
  }
  // The select isn't mounted, or it has no option for the loaded value yet.
  return currentLintConfig.game;
}

// The Settings select only lists games the linter can actually check. A
// project created with `init --game starfield` still has to display and
// preserve that value until the user picks Skyrim or Fallout 4.
function reflectGameControl(game: string) {
  if (!gameEl) {
    return;
  }
  for (const option of Array.from(gameEl.querySelectorAll("option[data-unlisted-game]"))) {
    option.remove();
  }
  if (!Array.from(gameEl.options).some((option) => option.value === game)) {
    const option = document.createElement("option");
    option.value = game;
    option.textContent = game === "starfield" ? "Starfield" : game;
    option.setAttribute("data-unlisted-game", "");
    gameEl.append(option);
  }
  gameEl.value = game;
}

// Sets the Settings tab's game control and persists it the same way a manual
// change would. Used by the first-run picker after a preset (or a continue
// that names a game) has been chosen for a project that had no config yet.
export function selectGame(game: Game): Promise<void> {
  if (!gameEl || !isSelectableGame(game)) {
    return Promise.resolve();
  }
  gameEl.value = game;
  return handleLintConfigChanged();
}

export function lintConfigFromUI(): LintConfig {
  const indentation = indentationStyleEl?.value === "spaces" ? "space" : "tab";
  const cyclomaticComplexityWarning = Math.max(1, cyclomaticComplexityWarningEl?.valueAsNumber || 10);
  const rules = { ...DEFAULT_RULES };
  for (const key of RULE_KEYS) {
    rules[key] = ruleEls[key]?.checked ?? DEFAULT_RULES[key];
  }
  return {
    game: gameFromControls(),
    semicolon: semicolonStyleEl?.value === "require",
    indentation,
    indentation_width: Math.min(16, Math.max(1, indentationWidthEl?.valueAsNumber || 4)),
    identifier_casing: (identifierCasingStyleEl?.value as IdentifierCasingStyle | undefined) ?? "PascalCase",
    cyclomatic_complexity_warning: cyclomaticComplexityWarning,
    // Never below the warning threshold: an error severity that kicks in
    // before the warning one would make the two settings contradict each
    // other.
    cyclomatic_complexity_error: Math.max(
      cyclomaticComplexityWarning,
      cyclomaticComplexityErrorEl?.valueAsNumber || 20,
    ),
    type_casing: (typeCasingStyleEl?.value as TypeCasingStyle | undefined) ?? "PascalCase",
    named_arguments: (namedArgumentsStyleEl?.value as NamedArgumentsStyle | undefined) ?? "never",
    min_wait_interval: Math.max(
      0,
      minWaitIntervalEl && Number.isFinite(minWaitIntervalEl.valueAsNumber)
        ? minWaitIntervalEl.valueAsNumber
        : 0.1,
    ),
    magic_numbers: (magicNumbersModeEl?.value as MagicNumbersMode | undefined) ?? "loose",
    fail_on_warning: failOnWarningEl?.checked ?? false,
    fail_on_info: failOnInfoEl?.checked ?? false,
    bool_like_int: boolLikeIntEl?.checked ?? true,
    assume_auto_properties_filled: assumeAutoPropertiesFilledEl?.checked ?? false,
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
  gameEl = document.querySelector("#game-select");
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");
  typeCasingStyleEl = document.querySelector("#type-casing-style");
  identifierCasingStyleEl = document.querySelector("#identifier-casing-style");
  namedArgumentsStyleEl = document.querySelector("#named-arguments-style");
  magicNumbersModeEl = document.querySelector("#magic-numbers-mode");
  cyclomaticComplexityWarningEl = document.querySelector("#cyclomatic-complexity-warning");
  cyclomaticComplexityErrorEl = document.querySelector("#cyclomatic-complexity-error");
  minWaitIntervalEl = document.querySelector("#min-wait-interval");
  failOnWarningEl = document.querySelector("#fail-on-warning");
  failOnInfoEl = document.querySelector("#fail-on-info");
  boolLikeIntEl = document.querySelector("#bool-like-int");
  assumeAutoPropertiesFilledEl = document.querySelector("#assume-auto-properties-filled");
  ruleEls = Object.fromEntries(
    RULE_KEYS.map((key) => [key, document.querySelector<HTMLInputElement>(`#rule-${key}`)]),
  ) as Partial<Record<keyof LintRules, HTMLInputElement>>;

  gameEl?.addEventListener("change", handleLintConfigChanged);
  semicolonStyleEl?.addEventListener("change", handleLintConfigChanged);
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
    handleLintConfigChanged();
  });
  indentationWidthEl?.addEventListener("change", handleLintConfigChanged);
  typeCasingStyleEl?.addEventListener("change", handleLintConfigChanged);
  identifierCasingStyleEl?.addEventListener("change", handleLintConfigChanged);
  namedArgumentsStyleEl?.addEventListener("change", handleLintConfigChanged);
  magicNumbersModeEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityWarningEl?.addEventListener("change", handleLintConfigChanged);
  cyclomaticComplexityErrorEl?.addEventListener("change", handleLintConfigChanged);
  minWaitIntervalEl?.addEventListener("change", handleLintConfigChanged);
  failOnWarningEl?.addEventListener("change", handleLintConfigChanged);
  failOnInfoEl?.addEventListener("change", handleLintConfigChanged);
  boolLikeIntEl?.addEventListener("change", handleLintConfigChanged);
  assumeAutoPropertiesFilledEl?.addEventListener("change", handleLintConfigChanged);
  for (const key of RULE_KEYS) {
    ruleEls[key]?.addEventListener("change", handleLintConfigChanged);
  }
}

// Mirrors RULE_ID_TO_CONFIG_KEY in scripts/generate-config-types.mjs: the
// handful of rule ids whose config key isn't just hyphens-to-underscores.
const RULE_ID_TO_CONFIG_KEY: Record<string, keyof LintRules> = {
  "float-to-int": "float_int_conversion",
  "too-many-named-states": "too_many_states",
};

// Maps a hyphenated lint rule id (Diagnostic.rule) onto its LintRules key,
// or undefined when the id isn't a configurable papyrus-lints rule (e.g. a
// compiler diagnostic).
export function configKeyForRuleId(ruleId: string): keyof LintRules | undefined {
  const key = (RULE_ID_TO_CONFIG_KEY[ruleId] ?? ruleId.replace(/-/g, "_")) as keyof LintRules;
  return RULE_KEYS.includes(key) ? key : undefined;
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

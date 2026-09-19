import { markLintResultsStale } from "./drop";
import { loadLintConfig, loadLintConfigFromPath, saveLintConfig, saveLintConfigToPath } from "./config-io";
import { type IdentifierCasingStyle, type LintConfig, type LintRules, type MagicNumbersMode, type NamedArgumentsStyle, type TypeCasingStyle, DEFAULT_RULES, RULE_KEYS, currentLintConfig, setCurrentLintConfig } from "./config-types";
import { configPathOverride, currentProjectDir } from "./project-state";
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
export function lintConfigFromUI(): LintConfig {
  const indentation = indentationStyleEl?.value === "spaces" ? "space" : "tab";
  const cyclomaticComplexityWarning = Math.max(1, cyclomaticComplexityWarningEl?.valueAsNumber || 10);
  const rules = { ...DEFAULT_RULES };
  for (const key of RULE_KEYS) {
    rules[key] = ruleEls[key]?.checked ?? DEFAULT_RULES[key];
  }
  return {
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
export function handleLintConfigChanged() {
  setCurrentLintConfig(lintConfigFromUI());
  markLintResultsStale();
  const override = configPathOverride();
  if (override) {
    void saveLintConfigToPath(override, currentLintConfig);
  } else if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

// Loads `dir`'s lint configuration (or the `overridePath` file instead, if
// set) into currentLintConfig and reflects it onto the Settings tab. The
// project.ts counterpart to handleLintConfigChanged: called by useProjectDir
// when a project is (re)loaded, rather than in response to editing a
// formatting control.
export async function loadAndApplyLintConfig(dir: string, overridePath: string): Promise<void> {
  setCurrentLintConfig(overridePath ? await loadLintConfigFromPath(overridePath) : await loadLintConfig(dir));
  applyLintConfigToUI(currentLintConfig);
}

export function bindConfigSettings() {
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

// LintConfig types, load/save, and Settings tab formatting UI. Implementation
// lives in the sibling config-*.ts modules; this file is the production
// façade so callers don't have to know which slice they need.

export type {
  Game,
  IdentifierCasingStyle,
  LintConfig,
  LintRules,
  MagicNumbersMode,
  NamedArgumentsStyle,
  TypeCasingStyle,
} from "./config-types";
export { currentLintConfig } from "./config-types";
// `./config-types` is generated at build time from shared/rules/*.json,
// configuration/papyrus-lint.default.yaml, and configuration/lint-settings.json;
// run `npm run generate:config-types` (also hooked from dev/build/test/lint)
// after adding a rule or a config field.
export {
  applyLintConfigToUI,
  bindConfigSettings,
  configKeyForRuleId,
  disableRulesInLintConfig,
  handleLintConfigChanged,
  loadAndApplyLintConfig,
  selectGame,
} from "./config-ui";

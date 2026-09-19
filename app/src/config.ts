// LintConfig types, load/save, and Settings tab formatting UI. Implementation
// lives in the sibling config-*.ts modules; this file is the production
// façade so callers don't have to know which slice they need.

export type {
  IdentifierCasingStyle,
  LintConfig,
  LintRules,
  MagicNumbersMode,
  NamedArgumentsStyle,
  TypeCasingStyle,
} from "./config-types";
export { currentLintConfig } from "./config-types";
export {
  applyLintConfigToUI,
  bindConfigSettings,
  handleLintConfigChanged,
  loadAndApplyLintConfig,
} from "./config-ui";

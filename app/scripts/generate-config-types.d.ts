export const RULE_ID_TO_CONFIG_KEY: Record<string, string>;

export function configKeyFor(ruleId: string): string;

export function assembleRules(rulesDir: string): Array<{
  id: string;
  name?: string;
  description?: string;
  enabled_by_default?: boolean;
}>;

export function parseDefaultYaml(source: string): {
  top: Record<string, string>;
  rules: Array<[string, boolean]>;
};

export function parseYamlScalar(raw: string): string | number | boolean;

export function orderRules<T extends { id: string }>(
  rules: T[],
  fieldOrder: string[],
): T[];

export interface LintSettingSource {
  key: string;
  yaml_default: string;
  ts_type: string;
  ts_alias?: string;
  ts_alias_from?: "value" | "config";
  ui: {
    id: string;
    widget: string;
    mount: string;
    ui_order?: number;
    options?: Array<{ value: string; label: string; config?: string }>;
    [extra: string]: unknown;
  };
}

export function renderConfigTypes(
  rules: Array<{ id: string; name?: string; description?: string; enabled_by_default?: boolean }>,
  defaultYaml: string,
  settings: LintSettingSource[],
): string;

export function writeConfigTypes(options: {
  rulesDir: string;
  defaultYamlPath: string;
  settingsPath: string;
  outPath: string;
}): number;

export const RULE_ID_TO_CONFIG_KEY: Record<string, string>;
export const LINT_CONFIG_KEYS: string[];

export function configKeyFor(ruleId: string): string;

export function assembleRules(rulesDir: string): Array<{
  id: string;
  enabled_by_default?: boolean;
}>;

export function parseDefaultYaml(source: string): {
  top: Record<string, string>;
  rules: Array<[string, boolean]>;
};

export function orderRules<T extends { id: string }>(
  rules: T[],
  fieldOrder: string[],
): T[];

export function renderConfigTypes(
  rules: Array<{ id: string; enabled_by_default?: boolean }>,
  defaultYaml: string,
): string;

export function writeConfigTypes(options: {
  rulesDir: string;
  defaultYamlPath: string;
  outPath: string;
}): number;

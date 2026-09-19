#!/usr/bin/env node
// Generates app/src/config-types.ts from shared/rules/*.json and
// configuration/papyrus-lint.default.yaml. Mirrors papyrus-lints/build.rs
// writing Rules / default_rules() into $OUT_DIR.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const RULE_ID_TO_CONFIG_KEY = {
  "float-to-int": "float_int_conversion",
  "too-many-named-states": "too_many_states",
};

export const LINT_CONFIG_KEYS = [
  "semicolon",
  "indentation",
  "indentation_width",
  "identifier_casing",
  "cyclomatic_complexity_warning",
  "cyclomatic_complexity_error",
  "type_casing",
  "named_arguments",
  "min_wait_interval",
  "magic_numbers",
  "fail_on_warning",
  "fail_on_info",
  "bool_like_int",
  "assume_auto_properties_filled",
];

const STRING_LINT_CONFIG_KEYS = new Set([
  "indentation",
  "identifier_casing",
  "type_casing",
  "named_arguments",
  "magic_numbers",
]);

const LINT_CONFIG_FIELD_TYPES = {
  semicolon: "boolean",
  indentation: '"tab" | "space"',
  indentation_width: "number",
  identifier_casing: "IdentifierCasingStyle",
  cyclomatic_complexity_warning: "number",
  cyclomatic_complexity_error: "number",
  type_casing: "TypeCasingStyle",
  named_arguments: "NamedArgumentsStyle",
  min_wait_interval: "number",
  magic_numbers: "MagicNumbersMode",
  fail_on_warning: "boolean",
  fail_on_info: "boolean",
  bool_like_int: "boolean",
  assume_auto_properties_filled: "boolean",
};

const HEADER = `// Generated from \`shared/rules/*.json\` and
// \`configuration/papyrus-lint.default.yaml\` by
// \`app/scripts/generate-config-types.mjs\`. Do not edit by hand.

export type TypeCasingStyle = "PascalCase" | "camelCase" | "lowercase" | "UPPERCASE";
export type IdentifierCasingStyle = "camelCase" | "PascalCase" | "snake_case" | "CONSTANT_CASE";
export type NamedArgumentsStyle = "always" | "instead_of_defaults" | "never";
export type MagicNumbersMode = "loose" | "strict";
`;

export function configKeyFor(ruleId) {
  return RULE_ID_TO_CONFIG_KEY[ruleId] ?? ruleId.replaceAll("-", "_");
}

export function assembleRules(rulesDir) {
  const paths = fs
    .readdirSync(rulesDir)
    .filter((name) => name.endsWith(".json"))
    .sort()
    .map((name) => path.join(rulesDir, name));
  if (paths.length === 0) {
    throw new Error(`no rule files found in ${rulesDir}`);
  }
  return paths.map((filePath) => {
    const rule = JSON.parse(fs.readFileSync(filePath, "utf8"));
    const stem = path.basename(filePath, ".json");
    if (rule.id !== stem) {
      throw new Error(`${filePath}: \`id\` is ${JSON.stringify(rule.id)}, expected ${JSON.stringify(stem)} to match the file name`);
    }
    return rule;
  });
}

export function parseDefaultYaml(source) {
  const top = {};
  const rules = [];
  let inRules = false;
  for (const line of source.split("\n")) {
    const stripped = line.trim();
    if (!stripped || stripped.startsWith("#")) {
      continue;
    }
    if (!inRules) {
      if (stripped === "rules:") {
        inRules = true;
        continue;
      }
      const sep = stripped.indexOf(":");
      if (sep < 0) {
        throw new Error(`default YAML line is not key: value: ${JSON.stringify(line)}`);
      }
      top[stripped.slice(0, sep).trim()] = stripped.slice(sep + 1).trim();
      continue;
    }
    if (!line.startsWith("  ") || line.startsWith("   ")) {
      break;
    }
    const sep = stripped.indexOf(":");
    if (sep < 0) {
      throw new Error(`default YAML rules line is not key: value: ${JSON.stringify(line)}`);
    }
    rules.push([stripped.slice(0, sep).trim(), stripped.slice(sep + 1).trim() === "true"]);
  }
  if (rules.length === 0) {
    throw new Error("default YAML has no \`rules:\` entries");
  }
  return { top, rules };
}

export function orderRules(rules, fieldOrder) {
  const byKey = new Map();
  for (const rule of rules) {
    const key = configKeyFor(rule.id);
    if (byKey.has(key)) {
      throw new Error(`duplicate Rules field \`${key}\``);
    }
    byKey.set(key, rule);
  }
  const ordered = [];
  for (const key of fieldOrder) {
    const rule = byKey.get(key);
    if (!rule) {
      throw new Error(
        `configuration/papyrus-lint.default.yaml lists rules.${key} but shared/rules has no matching id`,
      );
    }
    byKey.delete(key);
    ordered.push(rule);
  }
  if (byKey.size > 0) {
    const missing = [...byKey.keys()].sort().join(", ");
    throw new Error(
      `configuration/papyrus-lint.default.yaml is missing rules: ${missing}; add them next to the other \`rules:\` keys`,
    );
  }
  return ordered;
}

function tsDefault(key, raw) {
  return STRING_LINT_CONFIG_KEYS.has(key) ? `"${raw}"` : raw;
}

export function renderConfigTypes(rules, defaultYaml) {
  const { top, rules: yamlRules } = parseDefaultYaml(defaultYaml);
  const missingTop = LINT_CONFIG_KEYS.filter((key) => !(key in top));
  if (missingTop.length > 0) {
    throw new Error(`default YAML is missing LintConfig keys: ${missingTop.join(", ")}`);
  }

  const ordered = orderRules(rules, yamlRules.map(([key]) => key));
  const yamlValues = Object.fromEntries(yamlRules);
  const lines = [HEADER, "export interface LintRules {"];
  const defaults = [];
  for (const rule of ordered) {
    const key = configKeyFor(rule.id);
    const enabled = rule.enabled_by_default !== false;
    if (yamlValues[key] !== enabled) {
      throw new Error(
        `rules.${key} is ${yamlValues[key]} in the default YAML but enabled_by_default is ${enabled} in shared/rules`,
      );
    }
    lines.push(`  ${key}: boolean;`);
    defaults.push(`  ${key}: ${enabled ? "true" : "false"},`);
  }
  lines.push("}", "", "export interface LintConfig {");
  for (const key of LINT_CONFIG_KEYS) {
    lines.push(`  ${key}: ${LINT_CONFIG_FIELD_TYPES[key]};`);
  }
  lines.push("  rules: LintRules;", "}", "");
  lines.push("export const DEFAULT_RULES: LintRules = {");
  lines.push(...defaults);
  lines.push("};", "");
  lines.push("export const DEFAULT_LINT_CONFIG: LintConfig = {");
  for (const key of LINT_CONFIG_KEYS) {
    lines.push(`  ${key}: ${tsDefault(key, top[key])},");
  }
  lines.push("  rules: DEFAULT_RULES,", "};", "");
  lines.push("export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];");
  lines.push("");
  lines.push("export let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;");
  lines.push("");
  lines.push("export function setCurrentLintConfig(config: LintConfig) {");
  lines.push("  currentLintConfig = config;");
  lines.push("}", "");
  return lines.join("\n");
}

export function writeConfigTypes({ rulesDir, defaultYamlPath, outPath }) {
  const rules = assembleRules(rulesDir);
  const defaultYaml = fs.readFileSync(defaultYamlPath, "utf8");
  const rendered = renderConfigTypes(rules, defaultYaml);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, rendered, "utf8");
  return rules.length;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const appDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
  const repoRoot = path.resolve(appDir, "..");
  const count = writeConfigTypes({
    rulesDir: path.join(repoRoot, "shared", "rules"),
    defaultYamlPath: path.join(repoRoot, "configuration", "papyrus-lint.default.yaml"),
    outPath: path.join(appDir, "src", "config-types.ts"),
  });
  console.log(`Wrote ${count} rule flags to app/src/config-types.ts.`);
}

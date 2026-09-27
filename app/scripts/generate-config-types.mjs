#!/usr/bin/env node
// Generates app/src/config-types.ts from shared/rules/*.json,
// configuration/papyrus-lint.default.yaml, and configuration/lint-settings.json.
// Mirrors papyrus-lints/build.rs writing Rules / default_rules() and Config
// into $OUT_DIR.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const RULE_ID_TO_CONFIG_KEY = {
  "float-to-int": "float_int_conversion",
  "too-many-named-states": "too_many_states",
};

const HEADER = [
  "// Generated from `shared/rules/*.json`,",
  "// `configuration/papyrus-lint.default.yaml`, and",
  "// `configuration/lint-settings.json` by",
  "// `app/scripts/generate-config-types.mjs`. Do not edit by hand.",
  "",
].join("\n");

const UI_PASSTHROUGH = [
  ["label", "label"],
  ["title", "title"],
  ["aria_label", "ariaLabel"],
  ["checkbox_label", "checkboxLabel"],
  ["options", "options"],
  ["true_value", "trueValue"],
  ["false_value", "falseValue"],
  ["min", "min"],
  ["max", "max"],
  ["step", "step"],
  ["clamp_min", "clampMin"],
  ["clamp_max", "clampMax"],
  ["clamp_min_from", "clampMinFrom"],
  ["disabled_by_default", "disabledByDefault"],
  ["enables_key", "enablesKey"],
  ["enables_when", "enablesWhen"],
  ["group", "group"],
  ["group_kind", "groupKind"],
  ["group_legend", "groupLegend"],
  ["group_prefix", "groupPrefix"],
  ["row", "row"],
  ["fill_selects", "fillSelects"],
  ["treat_zero_as_empty", "treatZeroAsEmpty"],
  ["ui_order", "uiOrder"],
];

export function configKeyFor(ruleId) {
  return RULE_ID_TO_CONFIG_KEY[ruleId] ?? ruleId.replaceAll("-", "_");
}

export function assembleRules(rulesDir) {
  const names = fs.readdirSync(rulesDir).filter((name) => name.endsWith(".json")).sort();
  if (names.length === 0) {
    throw new Error(`no rule files found in ${rulesDir}`);
  }
  return names.map((name) => {
    const filePath = path.join(rulesDir, name);
    const rule = JSON.parse(fs.readFileSync(filePath, "utf8"));
    const stem = path.basename(filePath, ".json");
    if (rule.id !== stem) {
      throw new Error(
        `${filePath}: id is ${JSON.stringify(rule.id)}, expected ${JSON.stringify(stem)} to match the file name`,
      );
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
    throw new Error("default YAML has no rules: entries");
  }
  return { top, rules };
}

export function orderRules(rules, fieldOrder) {
  const byKey = new Map();
  for (const rule of rules) {
    const key = configKeyFor(rule.id);
    if (byKey.has(key)) {
      throw new Error(`duplicate Rules field ${key}`);
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
      `configuration/papyrus-lint.default.yaml is missing rules: ${missing}; add them next to the other rules: keys`,
    );
  }
  return ordered;
}

export function parseYamlScalar(raw) {
  if (raw === "true") {
    return true;
  }
  if (raw === "false") {
    return false;
  }
  if (/^-?\d+(\.\d+)?$/.test(raw)) {
    return Number(raw);
  }
  return raw;
}

function assertSettings(settings, top) {
  if (!Array.isArray(settings) || settings.length === 0) {
    throw new Error("lint settings are empty");
  }
  const seen = new Set();
  for (const setting of settings) {
    if (seen.has(setting.key)) {
      throw new Error(`duplicate lint setting ${setting.key}`);
    }
    seen.add(setting.key);
    if (typeof setting.yaml_default !== "string" || typeof setting.ts_type !== "string") {
      throw new Error(`lint setting ${setting.key} is missing yaml_default or ts_type`);
    }
    if (!setting.ui?.id || !setting.ui.widget || !setting.ui.mount) {
      throw new Error(`lint setting ${setting.key} is missing ui.id, ui.widget, or ui.mount`);
    }
    if (!(setting.key in top)) {
      throw new Error(`default YAML is missing ${setting.key}`);
    }
    if (top[setting.key] !== setting.yaml_default) {
      throw new Error(
        `default YAML ${setting.key} is ${top[setting.key]} but lint-settings says ${setting.yaml_default}`,
      );
    }
  }
}

function tsDefault(setting, raw) {
  return setting.ts_type === "boolean" || setting.ts_type === "number" ? raw : JSON.stringify(raw);
}

function lintSettingForTs(setting) {
  const ui = setting.ui;
  const out = {
    key: setting.key,
    id: ui.id,
    widget: ui.widget,
    mount: ui.mount,
    defaultValue: parseYamlScalar(setting.yaml_default),
  };
  for (const [from, to] of UI_PASSTHROUGH) {
    if (ui[from] !== undefined) {
      out[to] = ui[from];
    }
  }
  return out;
}

function aliasLines(settings) {
  const lines = [];
  const seen = new Set();
  for (const setting of settings) {
    if (!setting.ts_alias || seen.has(setting.ts_alias)) {
      continue;
    }
    seen.add(setting.ts_alias);
    const from = setting.ts_alias_from === "config" ? "config" : "value";
    const values = (setting.ui.options ?? []).map((option) => option[from] ?? option.value);
    if (values.length === 0) {
      throw new Error(`lint setting ${setting.key} declares ts_alias ${setting.ts_alias} without options`);
    }
    lines.push(
      `export type ${setting.ts_alias} = ${values.map((value) => JSON.stringify(value)).join(" | ")};`,
    );
    if (setting.key === "game") {
      lines.push(`export const SELECTABLE_GAMES = ${JSON.stringify(values)} as const;`);
    }
  }
  if (lines.length > 0) {
    lines.push("");
  }
  return lines;
}

export function renderConfigTypes(rules, defaultYaml, settings) {
  const parsed = parseDefaultYaml(defaultYaml);
  const top = parsed.top;
  const yamlRules = parsed.rules;
  assertSettings(settings, top);

  const ordered = orderRules(
    rules,
    yamlRules.map((entry) => entry[0]),
  );
  const yamlValues = Object.fromEntries(yamlRules);
  const lines = [HEADER, ...aliasLines(settings), "export interface LintRules {"];
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
  lines.push("}");
  lines.push("");
  lines.push("export interface LintConfig {");
  for (const setting of settings) {
    lines.push(`  ${setting.key}: ${setting.ts_type};`);
  }
  lines.push("  rules: LintRules;");
  lines.push("}");
  lines.push("");
  lines.push("export const DEFAULT_RULES: LintRules = {");
  lines.push(...defaults);
  lines.push("};");
  lines.push("");
  lines.push("export const DEFAULT_LINT_CONFIG: LintConfig = {");
  for (const setting of settings) {
    lines.push(`  ${setting.key}: ${tsDefault(setting, top[setting.key])},`);
  }
  lines.push("  rules: DEFAULT_RULES,");
  lines.push("};");
  lines.push("");
  lines.push("export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];");
  lines.push("");
  lines.push("export interface RuleSetting {");
  lines.push("  key: keyof LintRules;");
  lines.push("  id: string;");
  lines.push("  name: string;");
  lines.push("  description: string;");
  lines.push("}");
  lines.push("");
  lines.push("export const RULE_SETTINGS: readonly RuleSetting[] = [");
  for (const rule of ordered) {
    if (typeof rule.name !== "string" || rule.name.length === 0) {
      throw new Error(`shared/rules/${rule.id}.json is missing name`);
    }
    if (typeof rule.description !== "string" || rule.description.length === 0) {
      throw new Error(`shared/rules/${rule.id}.json is missing description`);
    }
    const key = configKeyFor(rule.id);
    lines.push(
      `  { key: ${JSON.stringify(key)}, id: ${JSON.stringify(rule.id)}, name: ${JSON.stringify(rule.name)}, description: ${JSON.stringify(rule.description)} },`,
    );
  }
  lines.push("];");
  lines.push("");
  lines.push("export interface LintSettingOption {");
  lines.push("  value: string;");
  lines.push("  label: string;");
  lines.push("  config?: string;");
  lines.push("}");
  lines.push("");
  lines.push("export interface LintSetting {");
  lines.push("  key: keyof Omit<LintConfig, \"rules\">;");
  lines.push("  id: string;");
  lines.push("  widget: \"game\" | \"select\" | \"bool-select\" | \"mapped-select\" | \"number\" | \"checkbox\";");
  lines.push("  mount: string;");
  lines.push("  defaultValue: string | number | boolean;");
  lines.push("  label?: string;");
  lines.push("  title?: string;");
  lines.push("  ariaLabel?: string;");
  lines.push("  checkboxLabel?: string;");
  lines.push("  options?: readonly LintSettingOption[];");
  lines.push("  trueValue?: string;");
  lines.push("  falseValue?: string;");
  lines.push("  min?: number;");
  lines.push("  max?: number;");
  lines.push("  step?: number;");
  lines.push("  clampMin?: number;");
  lines.push("  clampMax?: number;");
  lines.push("  clampMinFrom?: keyof Omit<LintConfig, \"rules\">;");
  lines.push("  disabledByDefault?: boolean;");
  lines.push("  enablesKey?: keyof Omit<LintConfig, \"rules\">;");
  lines.push("  enablesWhen?: string;");
  lines.push("  group?: string;");
  lines.push("  groupKind?: \"fieldset\" | \"indentation\";");
  lines.push("  groupLegend?: string;");
  lines.push("  groupPrefix?: string;");
  lines.push("  row?: boolean;");
  lines.push("  fillSelects?: readonly string[];");
  lines.push("  treatZeroAsEmpty?: boolean;");
  lines.push("  uiOrder?: number;");
  lines.push("}");
  lines.push("");
  lines.push("export const LINT_SETTINGS: readonly LintSetting[] = [");
  const uiOrdered = [...settings].sort((left, right) => (left.ui.ui_order ?? 0) - (right.ui.ui_order ?? 0));
  for (const setting of uiOrdered) {
    lines.push(`  ${JSON.stringify(lintSettingForTs(setting))},`);
  }
  lines.push("];");
  lines.push("");
  lines.push("export let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;");
  lines.push("");
  lines.push("export function setCurrentLintConfig(config: LintConfig) {");
  lines.push("  currentLintConfig = config;");
  lines.push("}");
  lines.push("");
  return lines.join("\n");
}

export function writeConfigTypes(options) {
  const rules = assembleRules(options.rulesDir);
  const defaultYaml = fs.readFileSync(options.defaultYamlPath, "utf8");
  const settingsFile = JSON.parse(fs.readFileSync(options.settingsPath, "utf8"));
  const rendered = renderConfigTypes(rules, defaultYaml, settingsFile.settings);
  fs.mkdirSync(path.dirname(options.outPath), { recursive: true });
  fs.writeFileSync(options.outPath, rendered, "utf8");
  return rules.length;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const appDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
  const repoRoot = path.resolve(appDir, "..");
  const count = writeConfigTypes({
    rulesDir: path.join(repoRoot, "shared", "rules"),
    defaultYamlPath: path.join(repoRoot, "configuration", "papyrus-lint.default.yaml"),
    settingsPath: path.join(repoRoot, "configuration", "lint-settings.json"),
    outPath: path.join(appDir, "src", "config-types.ts"),
  });
  console.log(`Wrote ${count} rule flags to app/src/config-types.ts.`);
}

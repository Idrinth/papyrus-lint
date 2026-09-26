import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { assembleRules, configKeyFor, renderConfigTypes } from "./generate-config-types.mjs";

const SAMPLE_SETTINGS = [
  {
    key: "game",
    yaml_default: "skyrim",
    ts_type: "Game",
    ts_alias: "Game",
    ui: {
      id: "game-select",
      widget: "game",
      mount: "lint-config-game",
      ui_order: 0,
      label: "Target game",
      options: [
        { value: "skyrim", label: "Skyrim" },
        { value: "fallout4", label: "Fallout 4" },
      ],
    },
  },
  {
    key: "semicolon",
    yaml_default: "false",
    ts_type: "boolean",
    ui: { id: "semicolon-style", widget: "bool-select", mount: "lint-config-settings", ui_order: 10, true_value: "require", false_value: "forbid" },
  },
  {
    key: "indentation",
    yaml_default: "tab",
    ts_type: "\"tab\" | \"space\"",
    ui: { id: "indentation-style", widget: "mapped-select", mount: "lint-config-settings", ui_order: 20 },
  },
  {
    key: "indentation_width",
    yaml_default: "4",
    ts_type: "number",
    ui: { id: "indentation-width", widget: "number", mount: "lint-config-settings", ui_order: 30 },
  },
  {
    key: "max_line_length",
    yaml_default: "120",
    ts_type: "number",
    ui: { id: "max-line-length", widget: "number", mount: "lint-config-settings", ui_order: 40 },
  },
  {
    key: "identifier_casing",
    yaml_default: "PascalCase",
    ts_type: "IdentifierCasingStyle",
    ts_alias: "IdentifierCasingStyle",
    ui: {
      id: "identifier-casing-style",
      widget: "select",
      mount: "lint-config-settings",
      ui_order: 60,
      options: [
        { value: "camelCase", label: "camelCase" },
        { value: "PascalCase", label: "PascalCase" },
        { value: "snake_case", label: "snake_case" },
        { value: "CONSTANT_CASE", label: "CONSTANT_CASE" },
      ],
    },
  },
  {
    key: "cyclomatic_complexity_warning",
    yaml_default: "10",
    ts_type: "number",
    ui: { id: "cyclomatic-complexity-warning", widget: "number", mount: "lint-config-settings", ui_order: 90 },
  },
  {
    key: "cyclomatic_complexity_error",
    yaml_default: "20",
    ts_type: "number",
    ui: { id: "cyclomatic-complexity-error", widget: "number", mount: "lint-config-settings", ui_order: 100 },
  },
  {
    key: "type_casing",
    yaml_default: "PascalCase",
    ts_type: "TypeCasingStyle",
    ts_alias: "TypeCasingStyle",
    ui: {
      id: "type-casing-style",
      widget: "select",
      mount: "lint-config-settings",
      ui_order: 50,
      options: [
        { value: "PascalCase", label: "PascalCase" },
        { value: "camelCase", label: "camelCase" },
        { value: "lowercase", label: "lowercase" },
        { value: "UPPERCASE", label: "UPPERCASE" },
      ],
    },
  },
  {
    key: "named_arguments",
    yaml_default: "never",
    ts_type: "NamedArgumentsStyle",
    ts_alias: "NamedArgumentsStyle",
    ui: {
      id: "named-arguments-style",
      widget: "select",
      mount: "lint-config-settings",
      ui_order: 70,
      options: [
        { value: "always", label: "Always" },
        { value: "instead_of_defaults", label: "Instead of defaults" },
        { value: "never", label: "Never" },
      ],
    },
  },
  {
    key: "min_wait_interval",
    yaml_default: "0.1",
    ts_type: "number",
    ui: { id: "min-wait-interval", widget: "number", mount: "lint-config-settings", ui_order: 110 },
  },
  {
    key: "magic_numbers",
    yaml_default: "loose",
    ts_type: "MagicNumbersMode",
    ts_alias: "MagicNumbersMode",
    ui: {
      id: "magic-numbers-mode",
      widget: "select",
      mount: "lint-config-settings",
      ui_order: 80,
      options: [
        { value: "loose", label: "Loose" },
        { value: "strict", label: "Strict" },
      ],
    },
  },
  {
    key: "fail_on_warning",
    yaml_default: "false",
    ts_type: "boolean",
    ui: { id: "fail-on-warning", widget: "checkbox", mount: "lint-config-settings", ui_order: 120 },
  },
  {
    key: "fail_on_info",
    yaml_default: "false",
    ts_type: "boolean",
    ui: { id: "fail-on-info", widget: "checkbox", mount: "lint-config-settings", ui_order: 130 },
  },
  {
    key: "bool_like_int",
    yaml_default: "true",
    ts_type: "boolean",
    ui: { id: "bool-like-int", widget: "checkbox", mount: "lint-config-settings", ui_order: 140 },
  },
  {
    key: "assume_auto_properties_filled",
    yaml_default: "false",
    ts_type: "boolean",
    ui: { id: "assume-auto-properties-filled", widget: "checkbox", mount: "lint-config-settings", ui_order: 150 },
  },
];

function yamlFor(...ruleLines) {
  const top = SAMPLE_SETTINGS.map((setting) => `${setting.key}: ${setting.yaml_default}`).join("\n");
  return `${top}\nrules:\n${ruleLines.map((line) => `  ${line}\n`).join("")}`;
}

function render(rules, yaml) {
  return renderConfigTypes(rules, yaml, SAMPLE_SETTINGS);
}

describe("generate-config-types", () => {
  it("maps hyphenated ids onto Rules field names", () => {
    expect(configKeyFor("trailing-whitespace")).toBe("trailing_whitespace");
    expect(configKeyFor("float-to-int")).toBe("float_int_conversion");
    expect(configKeyFor("too-many-named-states")).toBe("too_many_states");
  });

  it("renders rules in YAML order with enabled_by_default", () => {
    const rendered = render(
      [
        { id: "comma-spacing", name: "Space after comma", description: "Requires whitespace after commas.", enabled_by_default: true },
        { id: "property-sorting", name: "Property sorting", description: "Flags unsorted properties.", enabled_by_default: false },
      ],
      yamlFor("comma_spacing: true", "property_sorting: false"),
    );

    expect(rendered).toContain(
      "export interface LintRules {\n  comma_spacing: boolean;\n  property_sorting: boolean;\n}",
    );
    expect(rendered).toContain("  comma_spacing: true,\n  property_sorting: false,\n");
    expect(rendered).toContain('indentation: "tab"');
    expect(rendered).toContain('game: "skyrim"');
    expect(rendered).toContain('export type Game = "skyrim" | "fallout4";');
    expect(rendered).toContain('"id":"semicolon-style"');
    expect(rendered).toContain("app/scripts/generate-config-types.mjs");
    expect(rendered).toContain(
      '{ key: "comma_spacing", id: "comma-spacing", name: "Space after comma", description: "Requires whitespace after commas." }',
    );
    expect(rendered).toContain(
      '{ key: "property_sorting", id: "property-sorting", name: "Property sorting", description: "Flags unsorted properties." }',
    );
  });

  it("rejects a YAML rule missing from shared/rules", () => {
    expect(() =>
      render(
        [{ id: "comma-spacing", name: "Space after comma", description: "Requires whitespace.", enabled_by_default: true }],
        yamlFor("comma_spacing: true", "missing_rule: true"),
      ),
    ).toThrow(/no matching id/);
  });

  it("rejects a shared rule missing from the YAML", () => {
    expect(() =>
      render(
        [
          { id: "comma-spacing", name: "Space after comma", description: "Requires whitespace.", enabled_by_default: true },
          { id: "unused-property", name: "Unused script properties", description: "Flags unused properties.", enabled_by_default: true },
        ],
        yamlFor("comma_spacing: true"),
      ),
    ).toThrow(/missing rules: unused_property/);
  });

  it("renders the repository lint settings against the default YAML and rules", () => {
    const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
    const settings = JSON.parse(
      readFileSync(path.join(repoRoot, "configuration/lint-settings.json"), "utf8"),
    ).settings;
    const rendered = renderConfigTypes(
      assembleRules(path.join(repoRoot, "shared/rules")),
      readFileSync(path.join(repoRoot, "configuration/papyrus-lint.default.yaml"), "utf8"),
      settings,
    );
    expect(rendered).toContain('"id":"semicolon-style"');
    expect(rendered).toContain("assume_auto_properties_filled: boolean;");
    expect(rendered).toContain(
      'export const SELECTABLE_GAMES = ["skyrim","fallout4","starfield"] as const;',
    );
  });

  it("assembles rule files and rejects an id/filename mismatch", () => {
    const directory = mkdtempSync(path.join(tmpdir(), "config-types-"));
    writeFileSync(path.join(directory, "comma-spacing.json"), JSON.stringify({ id: "comma-spacing" }));
    expect(assembleRules(directory)).toEqual([{ id: "comma-spacing" }]);

    writeFileSync(path.join(directory, "other-rule.json"), JSON.stringify({ id: "wrong-id" }));
    expect(() => assembleRules(directory)).toThrow(/expected "other-rule"/);
  });
});

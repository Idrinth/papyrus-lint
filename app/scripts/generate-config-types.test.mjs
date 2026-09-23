import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { assembleRules, configKeyFor, renderConfigTypes } from "./generate-config-types.mjs";

const MINIMAL_TOP = `game: skyrim
semicolon: false
indentation: tab
indentation_width: 4
identifier_casing: PascalCase
cyclomatic_complexity_warning: 10
cyclomatic_complexity_error: 20
type_casing: PascalCase
named_arguments: never
min_wait_interval: 0.1
magic_numbers: loose
fail_on_warning: false
fail_on_info: false
bool_like_int: true
assume_auto_properties_filled: false
`;

function yamlFor(...ruleLines) {
  return `${MINIMAL_TOP}rules:\n${ruleLines.map((line) => `  ${line}\n`).join("")}`;
}

describe("generate-config-types", () => {
  it("maps hyphenated ids onto Rules field names", () => {
    expect(configKeyFor("trailing-whitespace")).toBe("trailing_whitespace");
    expect(configKeyFor("float-to-int")).toBe("float_int_conversion");
    expect(configKeyFor("too-many-named-states")).toBe("too_many_states");
  });

  it("renders rules in YAML order with enabled_by_default", () => {
    const rendered = renderConfigTypes(
      [
        { id: "comma-spacing", enabled_by_default: true },
        { id: "property-sorting", enabled_by_default: false },
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
    expect(rendered).toContain("app/scripts/generate-config-types.mjs");
  });

  it("rejects a YAML rule missing from shared/rules", () => {
    expect(() =>
      renderConfigTypes(
        [{ id: "comma-spacing", enabled_by_default: true }],
        yamlFor("comma_spacing: true", "missing_rule: true"),
      ),
    ).toThrow(/no matching id/);
  });

  it("rejects a shared rule missing from the YAML", () => {
    expect(() =>
      renderConfigTypes(
        [
          { id: "comma-spacing", enabled_by_default: true },
          { id: "unused-property", enabled_by_default: true },
        ],
        yamlFor("comma_spacing: true"),
      ),
    ).toThrow(/missing rules: unused_property/);
  });

  it("assembles rule files and rejects an id/filename mismatch", () => {
    const directory = mkdtempSync(path.join(tmpdir(), "config-types-"));
    writeFileSync(path.join(directory, "comma-spacing.json"), JSON.stringify({ id: "comma-spacing" }));
    expect(assembleRules(directory)).toEqual([{ id: "comma-spacing" }]);

    writeFileSync(path.join(directory, "other-rule.json"), JSON.stringify({ id: "wrong-id" }));
    expect(() => assembleRules(directory)).toThrow(/expected "other-rule"/);
  });
});

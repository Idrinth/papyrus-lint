import { describe, expect, it } from "vitest";
import { hasFixableFindings, isFixableFinding } from "./finding-fixability";

describe("hasFixableFindings", () => {
  it("is true for trailing whitespace findings", () => {
    expect(
      hasFixableFindings([
        { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
      ]),
    ).toBe(true);
  });

  it("is true for semicolon findings", () => {
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Lines should end with a semicolon", rule: "semicolon" }]),
    ).toBe(true);
  });

  it("is true for indentation findings", () => {
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Wrong indentation", rule: "indentation" }]),
    ).toBe(true);
  });

  it("is false when no findings are auto-fixable", () => {
    expect(hasFixableFindings([{ line: 1, column: 1, message: "[error] forbidden function used" }])).toBe(false);
  });

  it("is false for a fixable rule whose finding notes it has no automatic fix", () => {
    expect(
      hasFixableFindings([
        { line: 1, column: 1, message: "[warning] Bad casing (no automatic fix)", rule: "type-casing" },
      ]),
    ).toBe(false);
  });

  it("is false for an empty findings list", () => {
    expect(hasFixableFindings([])).toBe(false);
  });
});

describe("isFixableFinding", () => {
  it("is true for a finding whose rule has an automatic fix", () => {
    expect(isFixableFinding({ line: 1, column: 1, message: "[warning] trailing", rule: "trailing-whitespace" })).toBe(
      true,
    );
  });

  it("is true for a slow-function finding", () => {
    expect(
      isFixableFinding({ line: 1, column: 1, message: "[info] use the faster call", rule: "slow-functions" }),
    ).toBe(true);
  });

  it("is false for a finding whose rule has no automatic fix", () => {
    expect(
      isFixableFinding({ line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" }),
    ).toBe(false);
  });

  it("is false for a finding with no rule at all", () => {
    expect(isFixableFinding({ line: 1, column: 1, message: "[error] bad" })).toBe(false);
  });

  it("is true for an unused-import finding, since its fix removes the whole line", () => {
    expect(
      isFixableFinding({
        line: 3,
        column: 1,
        message: "[warning] Import 'Helpers' is never used: none of its Global functions are called unqualified anywhere in this script",
        rule: "unused-import",
      }),
    ).toBe(true);
  });

  it("is false for a type-casing finding its own message says has no automatic fix", () => {
    expect(
      isFixableFinding({
        line: 1,
        column: 1,
        message:
          "[warning] Script name 'IDR__TIF__050000F5' does not follow the configured PascalCase casing (fixing this would rename the script, so no automatic fix is applied)",
        rule: "type-casing",
      }),
    ).toBe(false);
  });

  it("is true for a type-casing finding a letter-casing-only rewrite can fix", () => {
    expect(
      isFixableFinding({
        line: 1,
        column: 1,
        message: "[warning] Script name 'myQuestScript' does not follow the configured PascalCase casing",
        rule: "type-casing",
      }),
    ).toBe(true);
  });
});


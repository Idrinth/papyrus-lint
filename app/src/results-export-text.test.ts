import { describe, expect, it } from "vitest";
import { formatIssuesAsText } from "./results-export-text";

describe("formatIssuesAsText", () => {
  it("renders one CLI-style diagnostic line per finding", () => {
    const text = formatIssuesAsText([
      {
        path: "scripts/source/A.psc",
        findings: [
          { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
          { line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" },
        ],
      },
    ]);

    expect(text).toBe(
      "scripts/source/A.psc:1:1: [warning] trailing whitespace (trailing-whitespace)\n" +
        "scripts/source/A.psc:5:3: [error] forbidden function used (forbidden-functions)",
    );
  });

  it("falls back to 'unknown' for a finding with no rule id", () => {
    const text = formatIssuesAsText([
      { path: "A.psc", findings: [{ line: 1, column: 1, message: "[error] compiler failure" }] },
    ]);

    expect(text).toBe("A.psc:1:1: [error] compiler failure (unknown)");
  });

  it("returns an empty string for no files", () => {
    expect(formatIssuesAsText([])).toBe("");
  });
});

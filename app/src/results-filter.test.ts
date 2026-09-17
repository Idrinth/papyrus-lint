import { describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import "./test/harness";
import { applyRuleTags, type Diagnostic, type PscParseOutcome } from "./main";
import { collectFilteredIssues, filterOutcomes } from "./results-filter";

describe("filterOutcomes", () => {
  function outcome(overrides: Partial<PscParseOutcome> = {}): PscParseOutcome {
    return { path: "/a.psc", ok: true, detail: 'parsed as "A"', findings: [], ...overrides };
  }

  const trailingWhitespace: Diagnostic = {
    line: 1,
    column: 1,
    message: "[warning] Line contains trailing whitespace",
    rule: "trailing-whitespace",
  };
  const forbiddenFunction: Diagnostic = {
    line: 5,
    column: 3,
    message: "[error] forbidden function used",
    rule: "forbidden-functions",
  };

  it("omits a successfully parsed file with no findings", () => {
    expect(filterOutcomes([outcome()])).toEqual([]);
  });

  it("keeps a file that failed to parse, even with no findings", () => {
    const failed = outcome({ ok: false, detail: "boom" });
    expect(filterOutcomes([failed])).toEqual([{ outcome: failed, findings: [] }]);
  });

  it("hands the list only the findings that pass the active severity filter", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      const source = outcome({ findings: [forbiddenFunction, trailingWhitespace] });
      expect(filterOutcomes([source])).toEqual([{ outcome: source, findings: [trailingWhitespace] }]);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-error")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));
    }
  });

  it("omits a successfully parsed file whose findings are all filtered out", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      expect(filterOutcomes([outcome({ findings: [forbiddenFunction] })])).toEqual([]);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-error")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));
    }
  });

  it("omits a file that doesn't match the filename filter", () => {
    const filterInput = document.querySelector<HTMLInputElement>("#filename-filter")!;
    filterInput.value = "*quest*";
    filterInput.dispatchEvent(new Event("input"));

    try {
      const quest = outcome({ path: "/MyQuestScript.psc", ok: false, detail: "boom" });
      const other = outcome({ path: "/OtherScript.psc", ok: false, detail: "boom" });
      expect(filterOutcomes([quest, other])).toEqual([{ outcome: quest, findings: [] }]);
    } finally {
      filterInput.value = "";
      filterInput.dispatchEvent(new Event("input"));
    }
  });

  it("hands the list only the findings whose rule is still selected", () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "comma-spacing", description: "Test description for comma spacing.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-comma-spacing" },
    ]);
    try {
      const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
      const trailingWhitespaceOption = [...select.options].find((option) => option.value === "trailing-whitespace")!;
      trailingWhitespaceOption.selected = false;
      select.dispatchEvent(new Event("change"));

      const commaSpacing: Diagnostic = {
        line: 2,
        column: 1,
        message: "[warning] missing space",
        rule: "comma-spacing",
      };
      const source = outcome({ findings: [trailingWhitespace, commaSpacing] });
      expect(filterOutcomes([source])).toEqual([{ outcome: source, findings: [commaSpacing] }]);
    } finally {
      applyRuleTags([]);
    }
  });

  it("passes the original outcome object through, so file-level actions still mutate it", () => {
    const source = outcome({ findings: [trailingWhitespace] });
    const [filtered] = filterOutcomes([source]);
    expect(filtered.outcome).toBe(source);
  });
});

describe("collectFilteredIssues", () => {
  it("omits a parse-failed file with no findings, even though filterOutcomes keeps it for the list", () => {
    const outcomes: PscParseOutcome[] = [{ path: "/broken.psc", ok: false, detail: "boom", findings: [] }];
    expect(filterOutcomes(outcomes)).toHaveLength(1);
    expect(collectFilteredIssues(outcomes)).toEqual([]);
  });
});

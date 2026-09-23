import { vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args), isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ show: showWindowMock }) }));

import { describe, expect, it } from "vitest";
import "./test/harness";
import { type Diagnostic, type PscParseOutcome } from "./backend-types";
import { massFixRuleCounts, massFixRuleDisplayName, renderMassFixList } from "./results-list-mass-fix";

describe("massFixRuleDisplayName", () => {
  it("returns the human-readable name for a known fixable rule", () => {
    expect(massFixRuleDisplayName("trailing-whitespace")).toBe("Trailing whitespace");
    expect(massFixRuleDisplayName("comma-spacing")).toBe("Space after comma");
    expect(massFixRuleDisplayName("unused-import")).toBe("Unused import");
  });

  it("falls back to the raw rule id for an unrecognized rule", () => {
    expect(massFixRuleDisplayName("some-future-rule")).toBe("some-future-rule");
  });
});

describe("massFixRuleCounts", () => {
  const trailingWhitespace: Diagnostic = {
    line: 1,
    column: 1,
    message: "[warning] Line contains trailing whitespace",
    rule: "trailing-whitespace",
  };
  const commaSpacing: Diagnostic = {
    line: 3,
    column: 5,
    message: "[warning] missing space after comma",
    rule: "comma-spacing",
  };
  const forbiddenFunction: Diagnostic = {
    line: 5,
    column: 1,
    message: "[error] forbidden function used",
    rule: "forbidden-functions",
  };

  it("counts fixable findings per rule across every outcome", () => {
    const outcomes: PscParseOutcome[] = [
      { path: "/a.psc", ok: false, detail: "", findings: [trailingWhitespace, forbiddenFunction] },
      { path: "/b.psc", ok: false, detail: "", findings: [trailingWhitespace, commaSpacing] },
    ];

    const counts = massFixRuleCounts(outcomes);

    expect(counts.get("trailing-whitespace")).toBe(2);
    expect(counts.get("comma-spacing")).toBe(1);
    expect(counts.has("forbidden-functions")).toBe(false);
  });

  it("returns an empty map when nothing is fixable", () => {
    const outcomes: PscParseOutcome[] = [{ path: "/a.psc", ok: false, detail: "", findings: [forbiddenFunction] }];

    expect(massFixRuleCounts(outcomes).size).toBe(0);
  });
});


describe("renderMassFixList", () => {
  const trailingWhitespace: Diagnostic = {
    line: 1,
    column: 1,
    message: "[warning] Line contains trailing whitespace",
    rule: "trailing-whitespace",
  };
  const commaSpacing: Diagnostic = {
    line: 3,
    column: 5,
    message: "[warning] missing space after comma",
    rule: "comma-spacing",
  };

  it("hides the panel when no outcome has a fixable finding", () => {
    renderMassFixList([{ path: "/a.psc", ok: true, detail: "", findings: [] }]);

    expect(document.querySelector<HTMLElement>("#psc-result-mass-fix")!.hidden).toBe(true);
  });

  it("lists each fixable rule, sorted by display name, with its count and a fix-all button", () => {
    const outcomes: PscParseOutcome[] = [
      { path: "/a.psc", ok: false, detail: "", findings: [trailingWhitespace] },
      { path: "/b.psc", ok: false, detail: "", findings: [trailingWhitespace, commaSpacing] },
    ];

    renderMassFixList(outcomes);

    const panel = document.querySelector<HTMLElement>("#psc-result-mass-fix")!;
    expect(panel.hidden).toBe(false);
    const items = [...document.querySelectorAll("#psc-result-mass-fix-list .psc-result__mass-fix-item")];
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toContain("Space after comma (1)");
    expect(items[0].querySelector("button")!.textContent).toBe("Fix this issue everywhere");
    expect(items[1].textContent).toContain("Trailing whitespace (2)");
    expect(items[1].querySelector("button")!.textContent).toBe("Fix all 2 in project");
  });
});

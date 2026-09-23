import { vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args), isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ show: showWindowMock }) }));

import { describe, expect, it } from "vitest";
import "./test/harness";
import { applyRuleTags } from "./main";
import { SEVERITIES } from "./main-severity";
import { switchTab } from "./main-tabs";
import { type PscParseOutcome } from "./backend-types";
import { renderPscResults } from "./results-list-render";

describe("renderPscResults", () => {
  function outcome(overrides: Partial<PscParseOutcome> = {}): PscParseOutcome {
    return { path: "/a.psc", ok: true, detail: 'parsed as "A"', findings: [], ...overrides };
  }
  it("hides a finding whose rule is deselected in its kind's 'Filter by rule' multiselect", () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "comma-spacing", description: "Test description for comma spacing.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-comma-spacing" },
    ]);
    try {
      const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
      const trailingWhitespaceOption = [...select.options].find((option) => option.value === "trailing-whitespace")!;
      trailingWhitespaceOption.selected = false;
      select.dispatchEvent(new Event("change"));

      renderPscResults([
        outcome({
          findings: [
            { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
            { line: 2, column: 1, message: "[warning] missing space", rule: "comma-spacing" },
          ],
        }),
      ]);
      const findingEls = document.querySelectorAll("#psc-result-list .psc-result__finding");
      expect(findingEls).toHaveLength(1);
      expect(findingEls[0].textContent).toContain("missing space");
    } finally {
      applyRuleTags([]);
    }
  });

  it("renderPscResults hides the panel entirely for an empty outcome list", () => {
    renderPscResults([{ findings: [{ line: 1, column: 1, message: "[error] x" }], ok: true, path: "/a.psc", detail: "" }]);
    renderPscResults([]);
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(true);
  });

  it("renderPscResults lists visible findings without forcing the lint tab", () => {
    switchTab("import");
    renderPscResults([
      outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] }),
      outcome({ path: "/b.psc" }),
    ]);

    // Rendering results (e.g. a progressive update mid-analysis, or a
    // fix/ignore action) must never yank the user back to the Lint results
    // tab if they've switched away - only an explicit switchTab call (see
    // handleDroppedPaths/relintCurrentFiles) should do that, and only once,
    // at the start of a fresh analysis.
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(true);
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  });

  it("renderPscResults respects the active severity filters", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      renderPscResults([outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] })]);

      // The only finding is filtered out, and the file itself parsed cleanly,
      // so it should be skipped entirely.
      expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(0);
    } finally {
      // activeSeverities is module state that outlives mountFixture(), same
      // as activeRules elsewhere in this file; restore it so it doesn't leak
      // into later tests.
      document.querySelector<HTMLInputElement>("#filter-error")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));
    }
  });

  it("refuses to leave every severity deselected, so an export can never be silently emptied by the severity filters", () => {
    // Every severity but the last one is still allowed to be deselected.
    for (const severity of SEVERITIES.slice(0, -1)) {
      const el = document.querySelector<HTMLInputElement>(`#filter-${severity}`)!;
      el.checked = false;
      el.dispatchEvent(new Event("change"));
      expect(el.checked).toBe(false);
    }

    // The last remaining severity refuses to uncheck.
    const lastEl = document.querySelector<HTMLInputElement>(`#filter-${SEVERITIES[SEVERITIES.length - 1]}`)!;
    lastEl.checked = false;
    lastEl.dispatchEvent(new Event("change"));
    expect(lastEl.checked).toBe(true);

    renderPscResults([outcome({ findings: [{ line: 1, column: 1, message: `[${SEVERITIES[SEVERITIES.length - 1]}] x` }] })]);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);

    // Restore every severity checkbox so this doesn't leak into later tests.
    for (const severity of SEVERITIES) {
      const el = document.querySelector<HTMLInputElement>(`#filter-${severity}`)!;
      el.checked = true;
      el.dispatchEvent(new Event("change"));
    }
  });

  it("renderPscResults respects the active tag filters", () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      // A second, untouched rule of another kind, so deselecting every
      // style rule below doesn't leave the global activeRules set empty -
      // the GUI refuses that regardless of which kind triggers it.
      { rule: "argument-types", description: "Test description for argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
    ]);
    try {
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.checked = false;
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.dispatchEvent(new Event("change"));

      renderPscResults([
        outcome({
          findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
        }),
      ]);

      // The only finding is a style-kind one, and style is now unchecked, so
      // the file (which otherwise parsed cleanly) should be skipped entirely.
      expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(0);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.dispatchEvent(new Event("change"));
      applyRuleTags([]);
    }
  });

  it("renderPscResults respects the filename filter input, typed live", () => {
    const filterInput = document.querySelector<HTMLInputElement>("#filename-filter")!;
    filterInput.value = "*quest*";
    filterInput.dispatchEvent(new Event("input"));

    try {
      renderPscResults([
        outcome({ path: "/MyQuestScript.psc", ok: false, detail: "boom" }),
        outcome({ path: "/OtherScript.psc", ok: false, detail: "boom" }),
      ]);

      const items = document.querySelectorAll("#psc-result-list > li");
      expect(items).toHaveLength(1);
      expect(items[0].textContent).toContain("MyQuestScript.psc");
    } finally {
      // currentFilenameFilter is module state that outlives mountFixture(),
      // so it must be cleared here or it would leak into later tests.
      filterInput.value = "";
      filterInput.dispatchEvent(new Event("input"));
    }
  });
});

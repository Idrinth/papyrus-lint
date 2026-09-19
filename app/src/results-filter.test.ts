import { afterEach, describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ show: showWindowMock }),
}));

import "./test/harness";
import { TAG_IMPORTANCES, type Diagnostic, type PscParseOutcome, type RuleTagsInfo } from "./backend";
import { applyRuleTags } from "./main";
import { collectFilteredIssues, filterOutcomes, matchesFilenameFilter, matchesTagFilters } from "./results-filter";
describe("matchesFilenameFilter", () => {
  it("matches everything when the pattern is empty or blank", () => {
    expect(matchesFilenameFilter("Scripts/MyQuest.psc", "")).toBe(true);
    expect(matchesFilenameFilter("Scripts/MyQuest.psc", "   ")).toBe(true);
  });

  it("matches a plain pattern as a case-insensitive substring", () => {
    expect(matchesFilenameFilter("Scripts/MyQuest.psc", "quest")).toBe(true);
    expect(matchesFilenameFilter("Scripts/MyQuest.psc", "QUEST")).toBe(true);
    expect(matchesFilenameFilter("Scripts/MyQuest.psc", "dialogue")).toBe(false);
  });

  it("treats * as matching any run of characters", () => {
    expect(matchesFilenameFilter("Scripts/MyQuestScript.psc", "*quest*")).toBe(true);
    expect(matchesFilenameFilter("Scripts/MyQuestScript.psc", "My*Script.psc")).toBe(true);
    expect(matchesFilenameFilter("Scripts/OtherScript.psc", "My*Script.psc")).toBe(false);
  });

  it("treats % as matching any run of characters, same as *", () => {
    expect(matchesFilenameFilter("Scripts/MyQuestScript.psc", "%quest%")).toBe(true);
  });

  it("treats ? as matching exactly one character", () => {
    expect(matchesFilenameFilter("Scripts/Quest1.psc", "Quest?.psc")).toBe(true);
    expect(matchesFilenameFilter("Scripts/Quest12.psc", "Quest?.psc")).toBe(false);
  });

  it("escapes regex-special characters in the literal parts of the pattern", () => {
    expect(matchesFilenameFilter("Scripts/A+B.psc", "A+B")).toBe(true);
    expect(matchesFilenameFilter("Scripts/AB.psc", "A+B")).toBe(false);
  });
});

describe("matchesTagFilters", () => {
  const styleLow: Diagnostic = { line: 1, column: 1, message: "x", rule: "trailing-whitespace" };
  const correctnessHigh: Diagnostic = { line: 1, column: 1, message: "x", rule: "argument-types" };

  // Populates ruleTagsByRule synchronously, with no `await` before it in the
  // calling test - see the loadRuleTags/applyRuleTags describe block above
  // for why that matters: applyRuleTags is also called (with an empty
  // array) from main.ts's own DOMContentLoaded handler once its startup
  // fetch resolves, and that resolution's timing relative to a test's own
  // hooks isn't guaranteed, so each test sets its own fixture as its first,
  // uninterrupted synchronous statement instead of relying on a shared
  // beforeEach.
  function useSampleTags() {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "argument-types", description: "Test description for argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
    ]);
  }

  // The active tag kind/importance/auto-fixable filters are module state
  // that outlives mountFixture(), same as activeSeverities/currentFilenameFilter
  // elsewhere in this file; restore every checkbox to its fixture default
  // (all checked except "Auto-fixable only") so a test that unchecks one
  // doesn't leak into a later test, in this describe block or any other.
  afterEach(() => {
    applyRuleTags([]);
    for (const id of [
      "filter-kind-style",
      "filter-kind-performance",
      "filter-kind-correctness",
      "filter-kind-maintainability",
      "filter-importance-low",
      "filter-importance-medium",
      "filter-importance-high",
    ]) {
      const el = document.querySelector<HTMLInputElement>(`#${id}`)!;
      el.checked = true;
      el.dispatchEvent(new Event("change"));
    }
    const autoFixableEl = document.querySelector<HTMLInputElement>("#filter-auto-fixable-only")!;
    autoFixableEl.checked = false;
    autoFixableEl.dispatchEvent(new Event("change"));
  });

  it("shows every finding before the rule list has loaded", () => {
    expect(matchesTagFilters(styleLow)).toBe(true);
    expect(matchesTagFilters(correctnessHigh)).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x" })).toBe(true);
  });

  it("shows every finding by default, tagged or not", () => {
    useSampleTags();
    expect(matchesTagFilters(styleLow)).toBe(true);
    expect(matchesTagFilters(correctnessHigh)).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x" })).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x", rule: "untagged-rule" })).toBe(true);
  });

  it("hides a finding whose kind header checkbox is unchecked", () => {
    useSampleTags();
    document.querySelector<HTMLInputElement>("#filter-kind-style")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-kind-style")!.dispatchEvent(new Event("change"));

    expect(matchesTagFilters(styleLow)).toBe(false);
    expect(matchesTagFilters(correctnessHigh)).toBe(true);
  });

  it("hides a finding whose rule is individually deselected in its kind's multiselect", () => {
    useSampleTags();
    const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    const trailingWhitespaceOption = [...select.options].find((option) => option.value === "trailing-whitespace")!;
    trailingWhitespaceOption.selected = false;
    select.dispatchEvent(new Event("change"));

    expect(matchesTagFilters(styleLow)).toBe(false);
    expect(matchesTagFilters(correctnessHigh)).toBe(true);
  });

  it("hides a finding whose importance is unchecked", () => {
    useSampleTags();
    document.querySelector<HTMLInputElement>("#filter-importance-high")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-importance-high")!.dispatchEvent(new Event("change"));

    expect(matchesTagFilters(correctnessHigh)).toBe(false);
    expect(matchesTagFilters(styleLow)).toBe(true);
  });

  it("refuses to leave every importance deselected, so an export can never be silently emptied by the importance filter", () => {
    useSampleTags();
    for (const importance of TAG_IMPORTANCES) {
      const el = document.querySelector<HTMLInputElement>(`#filter-importance-${importance}`)!;
      el.checked = false;
      el.dispatchEvent(new Event("change"));
    }

    // The last remaining importance ("high", the last entry in
    // TAG_IMPORTANCES) refuses to uncheck.
    const lastEl = document.querySelector<HTMLInputElement>("#filter-importance-high")!;
    expect(lastEl.checked).toBe(true);
    expect(matchesTagFilters(correctnessHigh)).toBe(true);
    expect(matchesTagFilters(styleLow)).toBe(false);
  });

  it("hides a non-auto-fixable finding when 'Auto-fixable only' is checked", () => {
    useSampleTags();
    document.querySelector<HTMLInputElement>("#filter-auto-fixable-only")!.checked = true;
    document.querySelector<HTMLInputElement>("#filter-auto-fixable-only")!.dispatchEvent(new Event("change"));

    expect(matchesTagFilters(correctnessHigh)).toBe(false);
    expect(matchesTagFilters(styleLow)).toBe(true);
    // An untagged finding (e.g. a compiler-reported diagnostic) is exempt
    // from the auto-fixable filter, same as from the kind/importance ones.
    expect(matchesTagFilters({ line: 1, column: 1, message: "x" })).toBe(true);
  });
});

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

describe("populateRuleFilterGroups (via applyRuleTags)", () => {
  const sampleTags: RuleTagsInfo[] = [
    { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
    { rule: "argument-types", description: "Test description for argument types.", kinds: ["performance", "correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
  ];

  // ruleTagsByRule/activeRules are module state that outlives mountFixture();
  // reset them so they don't leak into later tests - see the
  // loadRuleTags/applyRuleTags describe block above for why this matters.
  afterEach(() => {
    applyRuleTags([]);
  });

  it("populates each kind's rule filter select, sorted by rule id, all selected", () => {
    applyRuleTags(sampleTags);

    const styleSelect = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    expect([...styleSelect.options].map((option) => option.value)).toEqual(["trailing-whitespace"]);
    expect([...styleSelect.options].map((option) => option.textContent)).toEqual(["Trailing whitespace"]);
    expect([...styleSelect.options].every((option) => option.selected)).toBe(true);

    // A rule tagged with more than one kind (here "argument-types", tagged
    // both "performance" and "correctness") appears in each of its kinds'
    // own selects.
    const performanceSelect = document.querySelector<HTMLSelectElement>("#filter-rule-performance")!;
    const correctnessSelect = document.querySelector<HTMLSelectElement>("#filter-rule-correctness")!;
    expect([...performanceSelect.options].map((option) => option.value)).toEqual(["argument-types"]);
    expect([...correctnessSelect.options].map((option) => option.value)).toEqual(["argument-types"]);

    const maintainabilitySelect = document.querySelector<HTMLSelectElement>("#filter-rule-maintainability")!;
    expect(maintainabilitySelect.options).toHaveLength(0);
  });

  it("rebuilds every kind's select on a later call, dropping stale options", () => {
    applyRuleTags(sampleTags);
    applyRuleTags([sampleTags[0]]);

    const styleSelect = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    expect([...styleSelect.options].map((option) => option.value)).toEqual(["trailing-whitespace"]);
    const correctnessSelect = document.querySelector<HTMLSelectElement>("#filter-rule-correctness")!;
    expect(correctnessSelect.options).toHaveLength(0);
  });

  it("deselecting a multi-kind rule in one of its selects deselects it in the other too", () => {
    applyRuleTags(sampleTags);

    const performanceSelect = document.querySelector<HTMLSelectElement>("#filter-rule-performance")!;
    performanceSelect.options[0].selected = false;
    performanceSelect.dispatchEvent(new Event("change"));

    const correctnessSelect = document.querySelector<HTMLSelectElement>("#filter-rule-correctness")!;
    expect(correctnessSelect.options[0].selected).toBe(false);
  });

  it("unchecks a kind's header checkbox once every rule in its select is deselected, and marks it indeterminate for a partial selection", () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "comma-spacing", description: "Test description for comma spacing.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-comma-spacing" },
      // A rule of another kind, left selected throughout, so deselecting
      // every style rule below doesn't leave the global activeRules set
      // empty - the GUI refuses that regardless of which kind triggers it
      // (see the "never leaves every rule deselected" test below).
      { rule: "argument-types", description: "Test description for argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
    ]);
    const header = document.querySelector<HTMLInputElement>("#filter-kind-style")!;
    expect(header.checked).toBe(true);
    expect(header.indeterminate).toBe(false);

    const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    const trailingWhitespaceOption = [...select.options].find((option) => option.value === "trailing-whitespace")!;
    trailingWhitespaceOption.selected = false;
    select.dispatchEvent(new Event("change"));
    expect(header.checked).toBe(false);
    expect(header.indeterminate).toBe(true);

    const commaSpacingOption = [...select.options].find((option) => option.value === "comma-spacing")!;
    commaSpacingOption.selected = false;
    select.dispatchEvent(new Event("change"));
    expect(header.checked).toBe(false);
    expect(header.indeterminate).toBe(false);
  });

  it("checking a kind's header checkbox re-selects every rule in its select", () => {
    applyRuleTags([{ rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" }]);
    const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    select.options[0].selected = false;
    select.dispatchEvent(new Event("change"));

    const header = document.querySelector<HTMLInputElement>("#filter-kind-style")!;
    header.checked = true;
    header.dispatchEvent(new Event("change"));

    expect(select.options[0].selected).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toBe(true);
  });

  it("refuses to leave every rule deselected via a kind's multiselect, so an export can never be silently emptied by the rule filter", () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "argument-types", description: "Test description for argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
    ]);
    const styleSelect = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    styleSelect.options[0].selected = false;
    styleSelect.dispatchEvent(new Event("change"));
    // Allowed: a rule of another kind is still active.
    expect(styleSelect.options[0].selected).toBe(false);

    const correctnessSelect = document.querySelector<HTMLSelectElement>("#filter-rule-correctness")!;
    correctnessSelect.options[0].selected = false;
    correctnessSelect.dispatchEvent(new Event("change"));

    // Deselecting the very last active rule (across every kind, not just
    // this one's own select) is refused: it reverts back to selected.
    expect(correctnessSelect.options[0].selected).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x", rule: "argument-types" })).toBe(true);
  });

  it("refuses to leave every rule deselected via a kind's header checkbox", () => {
    applyRuleTags([{ rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" }]);
    const header = document.querySelector<HTMLInputElement>("#filter-kind-style")!;
    header.checked = false;
    header.dispatchEvent(new Event("change"));

    // The only known rule refuses to be fully deselected: the header and
    // its select both revert.
    expect(header.checked).toBe(true);
    const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    expect(select.options[0].selected).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toBe(true);
  });
});

describe("collectFilteredIssues", () => {
  it("omits a parse-failed file with no findings, even though filterOutcomes keeps it for the list", () => {
    const outcomes: PscParseOutcome[] = [{ path: "/broken.psc", ok: false, detail: "boom", findings: [] }];
    expect(filterOutcomes(outcomes)).toHaveLength(1);
    expect(collectFilteredIssues(outcomes)).toEqual([]);
  });

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

  it("groups the findings that pass the active filters by file path", () => {
    const outcomes: PscParseOutcome[] = [
      { path: "/somewhere/A.psc", ok: true, detail: "", findings: [trailingWhitespace] },
      { path: "/somewhere/B.psc", ok: true, detail: "", findings: [forbiddenFunction] },
    ];

    expect(collectFilteredIssues(outcomes)).toEqual([
      { path: "/somewhere/A.psc", findings: [trailingWhitespace] },
      { path: "/somewhere/B.psc", findings: [forbiddenFunction] },
    ]);
  });

  it("omits a file whose findings are all filtered out by the active severity filter", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      const outcomes: PscParseOutcome[] = [
        { path: "/a.psc", ok: true, detail: "", findings: [forbiddenFunction] },
        { path: "/b.psc", ok: true, detail: "", findings: [trailingWhitespace] },
      ];

      expect(collectFilteredIssues(outcomes)).toEqual([{ path: "/b.psc", findings: [trailingWhitespace] }]);
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
      const outcomes: PscParseOutcome[] = [
        { path: "/MyQuestScript.psc", ok: true, detail: "", findings: [trailingWhitespace] },
        { path: "/OtherScript.psc", ok: true, detail: "", findings: [forbiddenFunction] },
      ];

      expect(collectFilteredIssues(outcomes)).toEqual([{ path: "/MyQuestScript.psc", findings: [trailingWhitespace] }]);
    } finally {
      filterInput.value = "";
      filterInput.dispatchEvent(new Event("input"));
    }
  });

  it("omits a file with no findings at all, e.g. one that failed to parse", () => {
    const outcomes: PscParseOutcome[] = [{ path: "/broken.psc", ok: false, detail: "boom", findings: [] }];

    expect(collectFilteredIssues(outcomes)).toEqual([]);
  });

  it("returns an empty array when nothing is loaded", () => {
    expect(collectFilteredIssues([])).toEqual([]);
  });
});

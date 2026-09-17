import { afterEach, describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import { confirmDetectedConfig, invokeImplFor } from "./test/harness";
import {
  applyRuleTags,
  handleDroppedPaths,
  SEVERITIES,
  switchTab,
  type Diagnostic,
  type PscParseOutcome,
} from "./main";
import { DEFAULT_LINT_CONFIG } from "./config";
import { useProjectDir } from "./project";
import { aiConfiguration } from "./results-export-ai";
import {
  buildPscResultItem,
  formatIssuesForAi,
  handleCompileClick,
  handleExportAiClick,
  handleExportIssuesClick,
  handleFixClick,
  handleFixIssueClick,
  handleMassFixClick,
  massFixRuleCounts,
  massFixRuleDisplayName,
  renderMassFixList,
  renderPscResults,
  updateExportIssuesButtonState,
  type AiSource,
} from "./results-list";

describe("buildPscResultItem / renderPscResults", () => {
  function outcome(overrides: Partial<PscParseOutcome> = {}): PscParseOutcome {
    return { path: "/a.psc", ok: true, detail: 'parsed as "A"', findings: [], ...overrides };
  }

  it("skips a clean, successfully parsed file", () => {
    expect(buildPscResultItem(outcome())).toBeNull();
  });

  it("renders the findings it is given, without applying filters itself", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      const item = buildPscResultItem(
        outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] }),
      );
      expect(item!.querySelectorAll(".psc-result__finding")).toHaveLength(1);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-error")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));
    }
  });

  it("always shows a file that failed to parse, even with no findings", () => {
    const item = buildPscResultItem(outcome({ ok: false, detail: "boom" }));
    expect(item).not.toBeNull();
    expect(item!.classList.contains("psc-result__item--error")).toBe(true);
    expect(item!.textContent).toContain("boom");
  });

  it("shows a fix button only when findings are auto-fixable", () => {
    const fixable = buildPscResultItem(
      outcome({
        findings: [
          { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
        ],
      }),
    );
    expect(fixable!.querySelector(".psc-result__fix-button")).not.toBeNull();

    const unfixable = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "[error] forbidden function used" }] }),
    );
    expect(unfixable!.querySelector(".psc-result__fix-button")).toBeNull();
  });

  it("shows the path relative to the current project dir, when known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await useProjectDir("/a");

    const item = buildPscResultItem(outcome({ path: "/a/b.psc", ok: false, detail: "boom" }));
    expect(item!.textContent).toContain("b.psc: boom");
    expect(item!.textContent).not.toContain("/a/b.psc");
  });

  it("always shows a compile button, even for a file with no findings", () => {
    const item = buildPscResultItem(outcome({ ok: false, detail: "boom" }));
    expect(item!.querySelector(".psc-result__compile-button")).not.toBeNull();
    expect(item!.querySelector(".psc-result__compile-output")).not.toBeNull();
  });

  it("opens the code viewer when the result's View code button is clicked", async () => {
    invokeImplFor({ read_psc_file: () => "ScriptName A" });
    const item = buildPscResultItem(
      outcome({ ok: false, findings: [{ line: 1, column: 1, message: "[error] bad thing" }] }),
    );
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__view-button")!.click();

    await vi.waitFor(() => {
      expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(true);
    });
    expect(document.querySelector("#code-viewer-title")!.textContent).toMatch(/a\.psc$/);
    expect(document.querySelector("#code-viewer-line-1")!.classList).toContain("code-viewer__line--error");
  });

  it("runs all automatic fixes when the result's Apply fixes button is clicked", async () => {
    invokeImplFor({ repair_psc_file: () => [] });
    const result = outcome({
      findings: [
        { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
      ],
    });
    const item = buildPscResultItem(result);
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__fix-button")!.click();

    await vi.waitFor(() => expect(result.findings).toEqual([]));
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.any(Object));
  });

  it("compiles a file when the result's Compile button is clicked", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.", stderr: "" }),
    });
    const item = buildPscResultItem(outcome({ ok: false }));
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__compile-button")!.click();

    const output = item!.querySelector<HTMLElement>(".psc-result__compile-output")!;
    await vi.waitFor(() => expect(output.hidden).toBe(false));
    expect(output.textContent).toBe("Compilation succeeded.");
    expect(output.classList).toContain("psc-result__compile-output--ok");
  });

  it("renders one finding entry per finding, tagged with its severity", () => {
    const item = buildPscResultItem(
      outcome({
        findings: [
          { line: 3, column: 5, message: "[error] bad thing" },
          { line: 4, column: 1, message: "[warning] risky thing" },
        ],
      }),
    );
    const findingEls = item!.querySelectorAll(".psc-result__finding");
    expect(findingEls).toHaveLength(2);
    expect(findingEls[0].classList.contains("psc-result__finding--error")).toBe(true);
    expect(findingEls[0].textContent).toContain("line 3, col 5");
    expect(findingEls[1].classList.contains("psc-result__finding--warning")).toBe(true);
  });

  it("shows a finding's tag badges when its rule has known tag metadata", () => {
    // See the matchesTagFilters describe block above for why applyRuleTags
    // is called synchronously right here, with no `await` before it.
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
    ]);
    try {
      const item = buildPscResultItem(
        outcome({
          findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
        }),
      );
      const badges = item!.querySelectorAll(".psc-result__tag-badge");
      const badgeText = Array.from(badges).map((badge) => badge.textContent);
      expect(badgeText).toContain("style");
      expect(badgeText).toContain("low importance");
      expect(badgeText).toContain("auto-fixable");
      const docsLink = item!.querySelector<HTMLAnchorElement>(".psc-result__tag-badge--docs-link");
      expect(docsLink?.href).toBe("https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace");
      expect(docsLink?.target).toBe("_blank");
    } finally {
      applyRuleTags([]);
    }
  });

  it("omits the auto-fixable badge for a fixable rule's finding that its own message says can't be fixed", () => {
    applyRuleTags([{ rule: "type-casing", description: "Test description for type casing.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-type-casing" }]);
    try {
      const item = buildPscResultItem(
        outcome({
          findings: [
            {
              line: 1,
              column: 1,
              message:
                "[warning] Script name 'IDR__TIF__050000F5' does not follow the configured PascalCase casing (fixing this would rename the script, so no automatic fix is applied)",
              rule: "type-casing",
            },
          ],
        }),
      );
      const badges = item!.querySelectorAll(".psc-result__tag-badge");
      const badgeText = Array.from(badges).map((badge) => badge.textContent);
      expect(badgeText).toContain("style");
      expect(badgeText).not.toContain("auto-fixable");
      expect(item!.querySelector(".psc-result__finding-fix-button")).toBeNull();
    } finally {
      applyRuleTags([]);
    }
  });

  it("shows no tag badges for a finding whose rule has no known tag metadata", () => {
    const item = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "[error] compile error", rule: "compiler-error" }] }),
    );
    expect(item!.querySelector(".psc-result__tag-badge")).toBeNull();
  });

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

  it("shows a 'Fix this issue' button only on findings whose own rule is auto-fixable", () => {
    const item = buildPscResultItem(
      outcome({
        findings: [
          { line: 3, column: 5, message: "[warning] missing space", rule: "comma-spacing" },
          { line: 4, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
        ],
      }),
    );
    const findingEls = item!.querySelectorAll(".psc-result__finding");
    expect(findingEls[0].querySelector(".psc-result__finding-fix-button")).not.toBeNull();
    expect(findingEls[1].querySelector(".psc-result__finding-fix-button")).toBeNull();
  });

  it("clicking a finding's fix button does not also open the code viewer", () => {
    invokeImplFor({ repair_psc_finding: () => [] });
    const item = buildPscResultItem(
      outcome({
        findings: [{ line: 3, column: 5, message: "[warning] missing space", rule: "comma-spacing" }],
      }),
    );

    item!.querySelector<HTMLButtonElement>(".psc-result__finding-fix-button")!.click();

    expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(false);
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

describe("handleFixClick", () => {
  it("disables the button, applies the repair, and re-renders with updated findings", async () => {
    const remaining: Diagnostic[] = [];
    invokeImplFor({ repair_psc_file: () => remaining });

    const button = document.createElement("button");
    const outcome: PscParseOutcome = {
      path: "/a.psc",
      ok: true,
      detail: "",
      findings: [{ line: 1, column: 1, message: "Line contains trailing whitespace" }],
    };

    const promise = handleFixClick("/a.psc", outcome, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcome.findings).toEqual(remaining);
  });

  it("reports repair failures without rejecting", async () => {
    invokeMock.mockRejectedValue(new Error("repair failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const button = document.createElement("button");
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "parsed", findings: [] };

    await expect(handleFixClick("/a.psc", outcome, button)).resolves.toBeUndefined();
    expect(button.disabled).toBe(true);
  });
});

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

describe("formatIssuesForAi", () => {
  // ruleTagsByRule is module state that outlives mountFixture(); reset it so
  // it doesn't leak into later tests (see the loadRuleTags/applyRuleTags
  // describe block for why this matters).
  afterEach(() => {
    applyRuleTags([]);
  });

  it("wraps the findings in a tool/version/website/target_game/generated_at header, removes message severity prefixes, and includes rule_details", async () => {
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      { rule: "forbidden-functions", description: "Test description for forbidden functions.", kinds: ["performance", "correctness"], importance: "medium", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-forbidden-functions" },
    ]);
    invokeImplFor({ preview_repair_psc_line: () => null });

    const files = [
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
      {
        path: "B.psc",
        findings: [
          { line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" },
          { line: 6, column: 1, message: "[error] compiler failure", rule: "compiler-error" },
        ],
      },
    ];

    const withSourceOmitted = JSON.parse(await formatIssuesForAi(files, "1.2.3"));
    expect(withSourceOmitted).toEqual({
      $schema: "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json",
      header: {
        tool: "Papyrus Lint",
        version: "1.2.3",
        website: "https://papyrus-lint.idrinth.de",
        target_game: "Skyrim SE/AE",
        generated_at: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/),
      },
      configuration: aiConfiguration(DEFAULT_LINT_CONFIG),
      filters: {
        filename_pattern: "",
        severities: ["error", "warning", "info"],
        importances: ["low", "medium", "high"],
        rules: ["forbidden-functions", "trailing-whitespace"],
        auto_fixable_only: false,
      },
      findings: {
        files: [
          {
            path: "A.psc",
            severity_counts: { errors: 0, warnings: 1, info: 0 },
            rule_counts: { "trailing-whitespace": 1 },
            diagnostics: [
              {
                line: 1,
                column: 1,
                rule: "trailing-whitespace",
                level: "warning",
                message: "trailing whitespace",
                doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace",
              },
            ],
            source: null,
          },
          {
            path: "B.psc",
            severity_counts: { errors: 2, warnings: 0, info: 0 },
            rule_counts: { "compiler-error": 1, "forbidden-functions": 1 },
            diagnostics: [
              {
                line: 5,
                column: 3,
                rule: "forbidden-functions",
                level: "error",
                message: "forbidden function used",
                doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-forbidden-functions",
              },
              {
                line: 6,
                column: 1,
                rule: "compiler-error",
                level: "error",
                message: "compiler failure",
                doc_url: null,
                external: true,
                source: "compiler",
              },
            ],
            source: null,
          },
        ],
        total_diagnostics: 3,
        severity_counts: { errors: 2, warnings: 1, info: 0 },
        rule_counts: { "compiler-error": 1, "forbidden-functions": 1, "trailing-whitespace": 1 },
      },
      rule_details: [
        { rule: "forbidden-functions", description: "Test description for forbidden functions.", kinds: ["performance", "correctness"], importance: "medium", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-forbidden-functions" },
        { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
      ],
    });
  });

  it("leaves a message without a recognized severity prefix unchanged", async () => {
    const json = JSON.parse(
      await formatIssuesForAi(
        [{ path: "A.psc", findings: [{ line: 1, column: 1, message: "external diagnostic", rule: "some-rule" }] }],
        "1.0.0",
      ),
    );

    expect(json.findings.files[0].diagnostics[0].message).toBe("external diagnostic");
  });

  it("records the GUI filters active when the export is generated", async () => {
    applyRuleTags([
      { rule: "argument-types", description: "Argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-argument-types" },
      { rule: "trailing-whitespace", description: "Trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
    ]);
    const filename = document.querySelector<HTMLInputElement>("#filename-filter")!;
    filename.value = "*Quest?.psc";
    filename.dispatchEvent(new Event("input"));
    const severity = document.querySelector<HTMLInputElement>("#filter-info")!;
    severity.checked = false;
    severity.dispatchEvent(new Event("change"));
    const importance = document.querySelector<HTMLInputElement>("#filter-importance-high")!;
    importance.checked = false;
    importance.dispatchEvent(new Event("change"));
    const rule = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    rule.options[0].selected = false;
    rule.dispatchEvent(new Event("change"));
    const autoFixable = document.querySelector<HTMLInputElement>("#filter-auto-fixable-only")!;
    autoFixable.checked = true;
    autoFixable.dispatchEvent(new Event("change"));

    const json = JSON.parse(await formatIssuesForAi([], "1.0.0"));

    expect(json.filters).toEqual({
      filename_pattern: "*Quest?.psc",
      severities: ["error", "warning"],
      importances: ["low", "medium"],
      rules: ["argument-types"],
      auto_fixable_only: true,
    });

    filename.value = "";
    filename.dispatchEvent(new Event("input"));
    severity.checked = true;
    severity.dispatchEvent(new Event("change"));
    importance.checked = true;
    importance.dispatchEvent(new Event("change"));
    autoFixable.checked = false;
    autoFixable.dispatchEvent(new Event("change"));
  });

  it("attaches each file's source from the given sources map, by its display path", async () => {
    invokeImplFor({ preview_repair_psc_line: () => null });
    const files = [
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
      {
        path: "B.psc",
        findings: [{ line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" }],
      },
    ];
    const sources = new Map<string, AiSource>([["A.psc", { type: "content", content: "ScriptName A\n" }]]);

    const json = JSON.parse(await formatIssuesForAi(files, "1.0.0", sources));

    expect(json.findings.files).toEqual([
      expect.objectContaining({ path: "A.psc", source: { type: "content", content: "ScriptName A\n" } }),
      expect.objectContaining({ path: "B.psc", source: null }),
    ]);
  });

  it("reports the version as 'unknown' when none was given", async () => {
    expect(JSON.parse(await formatIssuesForAi([], "")).header.version).toBe("unknown");
  });

  it("omits a triggered rule from rule_details when no tag metadata is known for it", async () => {
    applyRuleTags([]);

    const json = JSON.parse(
      await formatIssuesForAi(
        [{ path: "A.psc", findings: [{ line: 1, column: 1, message: "[warning] x", rule: "some-rule" }] }],
        "1.0.0",
      ),
    );

    expect(json.rule_details).toEqual([]);
  });

  it("returns no rule_details for no files", async () => {
    expect(JSON.parse(await formatIssuesForAi([], "1.0.0")).rule_details).toEqual([]);
  });

  it("flags a compiler-reported diagnostic as external, leaving an ordinary lint finding untouched", async () => {
    const files = [
      {
        path: "A.psc",
        findings: [
          { line: 8, column: 3, message: "[error] no viable alternative at character ';'", rule: "compiler-error" },
          { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
        ],
      },
    ];

    const json = JSON.parse(await formatIssuesForAi(files, "1.0.0"));

    expect(json.findings.files[0].diagnostics).toEqual([
      {
        line: 1,
        column: 1,
        rule: "trailing-whitespace",
        level: "warning",
        message: "trailing whitespace",
        doc_url: null,
      },
      {
        line: 8,
        column: 3,
        rule: "compiler-error",
        level: "error",
        message: "no viable alternative at character ';'",
        doc_url: null,
        external: true,
        source: "compiler",
      },
    ]);
  });

  it("attaches a repair preview to a finding whose rule has an automatic fix", async () => {
    invokeImplFor({
      preview_repair_psc_line: (args) => {
        const { rule, line } = args as { rule: string; line: number };
        return rule === "trailing-whitespace" && line === 1 ? "clean line" : null;
      },
    });
    const files = [
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
    ];

    const json = JSON.parse(await formatIssuesForAi(files, "1.0.0"));

    expect(json.findings.files[0].diagnostics[0].repair).toBe("clean line");
    expect(invokeMock).toHaveBeenCalledWith(
      "preview_repair_psc_line",
      expect.objectContaining({ path: "A.psc", rule: "trailing-whitespace", line: 1 }),
    );
  });

  it("omits the repair field when no preview could be computed", async () => {
    invokeImplFor({ preview_repair_psc_line: () => null });
    const files = [
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
    ];

    const json = JSON.parse(await formatIssuesForAi(files, "1.0.0"));

    expect(json.findings.files[0].diagnostics[0]).not.toHaveProperty("repair");
  });

  it("never requests a repair preview for a rule with no automatic fix", async () => {
    const files = [
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" }],
      },
    ];

    await formatIssuesForAi(files, "1.0.0");

    expect(invokeMock).not.toHaveBeenCalledWith("preview_repair_psc_line", expect.anything());
  });

  it("never requests a repair preview for a fixable rule's finding that itself has no automatic fix", async () => {
    const files = [
      {
        path: "A.psc",
        findings: [
          {
            line: 1,
            column: 1,
            message: "[warning] rename to PascalCase (no automatic fix: name already used elsewhere)",
            rule: "type-casing",
          },
        ],
      },
    ];

    await formatIssuesForAi(files, "1.0.0");

    expect(invokeMock).not.toHaveBeenCalledWith("preview_repair_psc_line", expect.anything());
  });
});

describe("Export issues button", () => {
  const finding: Diagnostic = {
    line: 1,
    column: 1,
    message: "[warning] Line contains trailing whitespace",
    rule: "trailing-whitespace",
  };

  it("updateExportIssuesButtonState disables the button when nothing is currently filtered", () => {
    updateExportIssuesButtonState([{ path: "/a.psc", ok: false, detail: "boom", findings: [] }]);

    expect(document.querySelector<HTMLButtonElement>("#export-issues-button")!.disabled).toBe(true);
  });

  it("updateExportIssuesButtonState enables the button once a finding passes the active filters", () => {
    updateExportIssuesButtonState([{ path: "/a.psc", ok: true, detail: "", findings: [finding] }]);

    expect(document.querySelector<HTMLButtonElement>("#export-issues-button")!.disabled).toBe(false);
  });

  it("renderPscResults itself keeps the button's disabled state in sync", () => {
    renderPscResults([{ path: "/a.psc", ok: true, detail: "", findings: [finding] }]);
    expect(document.querySelector<HTMLButtonElement>("#export-issues-button")!.disabled).toBe(false);

    renderPscResults([{ path: "/a.psc", ok: false, detail: "boom", findings: [] }]);
    expect(document.querySelector<HTMLButtonElement>("#export-issues-button")!.disabled).toBe(true);
  });

  // handleExportIssuesClick reads currentPscOutcomes (the same module state
  // renderPscResults's own filter-change listeners re-render from), not
  // whatever's passed straight to renderPscResults in the tests above - so
  // it needs to be populated the same way the app does, via a real drop.
  async function populateCurrentPscOutcomes(findings: Diagnostic[]): Promise<void> {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => findings,
    });
    const pending = handleDroppedPaths(["/proj/scripts/source/A.psc"]);
    await confirmDetectedConfig();
    await pending;
  }

  it("handleExportIssuesClick downloads a .txt file by default", async () => {
    await populateCurrentPscOutcomes([finding]);

    const objectUrl = "blob:mock-url";
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue(objectUrl);
    const revokeObjectURL = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    vi.useFakeTimers();
    try {
      handleExportIssuesClick();

      expect(createObjectURL).toHaveBeenCalledTimes(1);
      const [blob] = createObjectURL.mock.calls[0] as [Blob];
      expect(blob.type).toBe("text/plain");
      expect(click).toHaveBeenCalledTimes(1);
      // The Blob URL is deliberately not revoked synchronously (see
      // downloadTextFile) so an in-progress download can't race it - it's
      // revoked once the event loop is free again.
      expect(revokeObjectURL).not.toHaveBeenCalled();
      vi.runAllTimers();
      expect(revokeObjectURL).toHaveBeenCalledWith(objectUrl);
    } finally {
      vi.useRealTimers();
    }
  });

  it("handleExportIssuesClick downloads a .json file when JSON is selected", async () => {
    await populateCurrentPscOutcomes([finding]);
    document.querySelector<HTMLSelectElement>("#export-format")!.value = "json";

    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:mock-url");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    handleExportIssuesClick();

    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    expect(blob.type).toBe("application/json");
  });

  it("handleExportIssuesClick does nothing when there's nothing currently filtered", async () => {
    await populateCurrentPscOutcomes([]);

    const createObjectURL = vi.spyOn(URL, "createObjectURL");

    handleExportIssuesClick();

    expect(createObjectURL).not.toHaveBeenCalled();
  });

  it("updateExportIssuesButtonState keeps the 'Export for AI' button in sync with 'Export issues'", () => {
    expect(document.querySelector<HTMLButtonElement>("#export-ai-button")!.title).toContain(
      "everything an AI assistant needs",
    );

    updateExportIssuesButtonState([{ path: "/a.psc", ok: false, detail: "boom", findings: [] }]);
    expect(document.querySelector<HTMLButtonElement>("#export-ai-button")!.disabled).toBe(true);

    updateExportIssuesButtonState([{ path: "/a.psc", ok: true, detail: "", findings: [finding] }]);
    expect(document.querySelector<HTMLButtonElement>("#export-ai-button")!.disabled).toBe(false);
  });

  it("handleExportAiClick downloads a JSON file carrying the running app's version and each file's source", async () => {
    await populateCurrentPscOutcomes([finding]);
    // populateCurrentPscOutcomes's own invokeImplFor call doesn't stub
    // get_app_version/read_psc_file, and handleExportAiClick also requests a
    // repair preview for the fixable finding above (see formatIssuesForAi);
    // overriding the mock again here only affects the lookups
    // handleExportAiClick itself makes, since nothing else calls invoke()
    // between here and the assertion below.
    invokeImplFor({
      get_app_version: () => "9.9.9",
      read_psc_file: () => "ScriptName A\n",
      preview_repair_psc_line: () => null,
    });

    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:mock-url");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    await handleExportAiClick();

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    expect(blob.type).toBe("application/json");
    const contents = JSON.parse(await blob.text());
    expect(contents.header).toEqual({
      tool: "Papyrus Lint",
      version: "9.9.9",
      website: "https://papyrus-lint.idrinth.de",
      target_game: "Skyrim SE/AE",
      generated_at: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/),
    });
    expect(contents.findings.files).toEqual([
      expect.objectContaining({ source: { type: "content", content: "ScriptName A\n" } }),
    ]);
  });

  it("handleExportAiClick still downloads a report when a file's source can't be read", async () => {
    await populateCurrentPscOutcomes([finding]);
    invokeImplFor({
      get_app_version: () => "9.9.9",
      read_psc_file: () => Promise.reject(new Error("boom")),
      preview_repair_psc_line: () => null,
    });

    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:mock-url");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    await handleExportAiClick();

    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    const contents = JSON.parse(await blob.text());
    expect(contents.findings.files[0].source).toEqual({ type: "error", message: "Error: boom" });
  });

  it("handleExportAiClick attaches an md5 hash instead of full content when 'Redact source' is checked", async () => {
    await populateCurrentPscOutcomes([finding]);
    invokeImplFor({
      get_app_version: () => "9.9.9",
      hash_psc_file_md5: () => "d41d8cd98f00b204e9800998ecf8427e",
      preview_repair_psc_line: () => null,
    });
    document.querySelector<HTMLInputElement>("#export-ai-hash-source")!.checked = true;

    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:mock-url");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    await handleExportAiClick();

    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    const contents = JSON.parse(await blob.text());
    expect(contents.findings.files[0].source).toEqual({
      type: "hash",
      algorithm: "md5",
      hash: "d41d8cd98f00b204e9800998ecf8427e",
    });
  });

  it("handleExportAiClick does nothing when there's nothing currently filtered", async () => {
    await populateCurrentPscOutcomes([]);

    const createObjectURL = vi.spyOn(URL, "createObjectURL");

    await handleExportAiClick();

    expect(createObjectURL).not.toHaveBeenCalled();
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

describe("handleMassFixClick", () => {
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

  it("repairs the named rule only in files that have it, leaving the rest untouched", async () => {
    const outcomeA: PscParseOutcome = { path: "/a.psc", ok: false, detail: "", findings: [trailingWhitespace] };
    const outcomeB: PscParseOutcome = {
      path: "/b.psc",
      ok: false,
      detail: "",
      findings: [trailingWhitespace, commaSpacing],
    };
    const outcomeC: PscParseOutcome = { path: "/c.psc", ok: false, detail: "", findings: [commaSpacing] };
    invokeImplFor({
      repair_psc_file_rule: (args) => {
        const { path } = args as { path: string };
        return path === "/a.psc" ? [] : [commaSpacing];
      },
    });

    const button = document.createElement("button");
    const promise = handleMassFixClick("trailing-whitespace", [outcomeA, outcomeB, outcomeC], button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcomeA.findings).toEqual([]);
    expect(outcomeB.findings).toEqual([commaSpacing]);
    expect(outcomeC.findings).toEqual([commaSpacing]);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({
      path: "/a.psc",
      rule: "trailing-whitespace",
    }));
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({
      path: "/b.psc",
      rule: "trailing-whitespace",
    }));
    expect(invokeMock).not.toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({ path: "/c.psc" }));
  });
});

describe("handleFixIssueClick", () => {
  function setup(findings: Diagnostic[]) {
    const button = document.createElement("button");
    const errorEl = document.createElement("span");
    errorEl.hidden = true;
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "", findings };
    return { button, errorEl, outcome };
  }

  it("disables the button, applies just that finding's fix, and re-renders with updated findings", async () => {
    const finding: Diagnostic = {
      line: 3,
      column: 5,
      message: "[warning] missing space after comma",
      rule: "comma-spacing",
    };
    const remaining: Diagnostic[] = [];
    invokeImplFor({ repair_psc_finding: () => remaining });
    const { button, errorEl, outcome } = setup([finding]);

    const promise = handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcome.findings).toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_finding", expect.objectContaining({
      rule: "comma-spacing",
      line: 3,
    }));
  });

  it("does nothing for a finding with no rule", async () => {
    const finding: Diagnostic = { line: 1, column: 1, message: "[error] bad" };
    const { button, errorEl, outcome } = setup([finding]);

    await handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);

    expect(invokeMock).not.toHaveBeenCalledWith("repair_psc_finding", expect.anything());
    expect(button.disabled).toBe(false);
  });

  it("shows the backend's error inline and re-enables the button instead of re-rendering", async () => {
    const finding: Diagnostic = {
      line: 4,
      column: 1,
      message: "[warning] out of order",
      rule: "property-sorting",
    };
    invokeImplFor({
      repair_psc_finding: () => Promise.reject(new Error('Fixing this issue would change other lines in the file; use "Apply fixes" instead.')),
    });
    const { button, errorEl, outcome } = setup([finding]);
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);

    expect(outcome.findings).toEqual([finding]);
    expect(button.disabled).toBe(false);
    expect(errorEl.hidden).toBe(false);
    expect(errorEl.textContent).toContain("Apply fixes");
  });
});

describe("handleCompileClick", () => {
  function setup() {
    const button = document.createElement("button");
    button.textContent = "Compile";
    const outputEl = document.createElement("pre");
    outputEl.hidden = true;
    return { button, outputEl };
  }

  it("disables the button while compiling and restores its label afterward", async () => {
    invokeImplFor({ compile_psc_file: () => ({ success: true, stdout: "", stderr: "" }) });
    const { button, outputEl } = setup();

    const promise = handleCompileClick("/a.psc", button, outputEl);
    expect(button.disabled).toBe(true);
    expect(button.textContent).toBe("Compiling…");
    await promise;

    expect(button.disabled).toBe(false);
    expect(button.textContent).toBe("Compile");
  });

  it("shows the compiler's output and marks success", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.\n", stderr: "" }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("Compilation succeeded.");
    expect(outputEl.classList.contains("psc-result__compile-output--ok")).toBe(true);
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(false);
  });

  it("reports when personal data was stripped from the compiled script", async () => {
    invokeImplFor({
      compile_psc_file: () => ({
        success: true,
        stdout: "Compilation succeeded.\n",
        stderr: "",
        personal_data_stripped: true,
      }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toBe(
      "Compilation succeeded.\n\nRemoved your username/computer name from the compiled script.",
    );
  });

  it("shows a default success message when the compiler produced no output", async () => {
    invokeImplFor({ compile_psc_file: () => ({ success: true, stdout: "", stderr: "" }) });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toBe("Compiled successfully.");
  });

  it("marks a compiler-reported failure and shows its stderr", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: false, stdout: "", stderr: "Broken.psc(3,1): error\n" }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toContain("Broken.psc(3,1): error");
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(true);
  });

  it("shows a failure to launch the compiler (e.g. no path configured) as an error", async () => {
    invokeMock.mockRejectedValue(new Error("No PapyrusCompiler.exe path is configured."));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toContain("No PapyrusCompiler.exe path is configured.");
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(true);
    expect(button.disabled).toBe(false);
  });
});

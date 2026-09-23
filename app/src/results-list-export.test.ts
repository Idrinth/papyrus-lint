import { vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args), isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ show: showWindowMock }) }));

import { afterEach, describe, expect, it } from "vitest";
import { confirmDetectedConfig, invokeImplFor } from "./test/harness";
import { applyRuleTags } from "./main";
import { handleDroppedPaths } from "./drop";
import { type Diagnostic } from "./backend";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { aiConfiguration } from "./results-export-ai";
import { formatIssuesForAi, handleExportAiClick, handleExportIssuesClick, updateExportIssuesButtonState } from "./results-list-export";
import { renderPscResults } from "./results-list-render";
import { type AiSource } from "./results-export-types";

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
      $schema: "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v4.schema.json",
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
            parser_errors: [],
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
            parser_errors: [],
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
      await handleExportIssuesClick();

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

    await handleExportIssuesClick();

    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    expect(blob.type).toBe("application/json");
  });

  it("handleExportIssuesClick does nothing when there's nothing currently filtered", async () => {
    await populateCurrentPscOutcomes([]);

    const createObjectURL = vi.spyOn(URL, "createObjectURL");

    await handleExportIssuesClick();

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

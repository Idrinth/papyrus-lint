import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const onDragDropEventMock = vi.fn();
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import { mountFixture } from "./test/fixture";
import {
  DEFAULT_LINT_CONFIG,
  DEFAULT_RULES,
  applyLintConfigToUI,
  buildPscResultItem,
  clearError,
  dirnameOf,
  escapeAttr,
  handleDroppedPaths,
  handleFixClick,
  handleLintConfigChanged,
  hasFixableFindings,
  isAchlistPath,
  isPscPath,
  lastProjectDir,
  levelOf,
  lintConfigFromUI,
  loadLintConfig,
  openCodeViewer,
  parsePscFiles,
  rememberProjectDir,
  renderPscResults,
  repairPscFile,
  saveLintConfig,
  severityOf,
  showError,
  showResult,
  switchTab,
  toggleCodeViewerFullscreen,
  useProjectDir,
  type Diagnostic,
  type LintConfig,
  type PscParseOutcome,
} from "./main";

function invokeImplFor(handlers: Record<string, (args: any) => unknown>) {
  invokeMock.mockImplementation((command: string, args: unknown) => {
    const handler = handlers[command];
    if (!handler) {
      return Promise.reject(new Error(`unexpected command: ${command}`));
    }
    return Promise.resolve(handler(args));
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  localStorage.clear();
  mountFixture();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("path helpers", () => {
  it("isAchlistPath matches .achlist regardless of case", () => {
    expect(isAchlistPath("C:/mods/list.achlist")).toBe(true);
    expect(isAchlistPath("C:/mods/list.ACHLIST")).toBe(true);
    expect(isAchlistPath("C:/mods/list.psc")).toBe(false);
  });

  it("isPscPath matches .psc regardless of case", () => {
    expect(isPscPath("Foo.psc")).toBe(true);
    expect(isPscPath("Foo.PSC")).toBe(true);
    expect(isPscPath("Foo.achlist")).toBe(false);
  });

  it("dirnameOf strips the final path component for both slash styles", () => {
    expect(dirnameOf("/a/b/c.achlist")).toBe("/a/b");
    expect(dirnameOf("C:\\a\\b\\c.achlist")).toBe("C:\\a\\b");
  });

  it("dirnameOf returns the whole path when there is no separator", () => {
    expect(dirnameOf("c.achlist")).toBe("c.achlist");
  });
});

describe("severity helpers", () => {
  it("levelOf extracts a recognized bracketed prefix", () => {
    expect(levelOf("[error] boom")).toBe("error");
    expect(levelOf("[warning] hmm")).toBe("warning");
    expect(levelOf("[info] fyi")).toBe("info");
  });

  it("levelOf returns null when there is no recognized prefix", () => {
    expect(levelOf("Line contains trailing whitespace")).toBeNull();
    expect(levelOf("[weird] not a real level")).toBeNull();
  });

  it("severityOf falls back to 'other' when there is no level prefix", () => {
    expect(severityOf("[error] boom")).toBe("error");
    expect(severityOf("no prefix here")).toBe("other");
  });
});

describe("escapeAttr", () => {
  it("escapes &, \", <, > for safe use inside an HTML attribute", () => {
    expect(escapeAttr(`a & b " <c> `)).toBe("a &amp; b &quot; &lt;c&gt; ");
  });
});

describe("hasFixableFindings", () => {
  it("is true for trailing whitespace findings", () => {
    expect(hasFixableFindings([{ line: 1, column: 1, message: "Line contains trailing whitespace" }])).toBe(true);
  });

  it("is true for semicolon findings", () => {
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Lines should end with a semicolon" }]),
    ).toBe(true);
  });

  it("is false when no findings are auto-fixable", () => {
    expect(hasFixableFindings([{ line: 1, column: 1, message: "[error] forbidden function used" }])).toBe(false);
  });

  it("is false for an empty findings list", () => {
    expect(hasFixableFindings([])).toBe(false);
  });
});

describe("project dir memory", () => {
  it("round-trips through localStorage", () => {
    expect(lastProjectDir()).toBeNull();
    rememberProjectDir("/some/project");
    expect(lastProjectDir()).toBe("/some/project");
  });

  it("lastProjectDir tolerates a broken localStorage", () => {
    const getItemSpy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    expect(lastProjectDir()).toBeNull();
    getItemSpy.mockRestore();
  });
});

describe("switchTab", () => {
  it("shows only the selected panel and marks its tab selected", () => {
    switchTab("settings");

    const importPanel = document.querySelector<HTMLElement>("#panel-import")!;
    const settingsPanel = document.querySelector<HTMLElement>("#panel-settings")!;
    const importTab = document.querySelector<HTMLButtonElement>("#tab-import")!;
    const settingsTab = document.querySelector<HTMLButtonElement>("#tab-settings")!;

    expect(settingsPanel.hidden).toBe(false);
    expect(importPanel.hidden).toBe(true);
    expect(settingsTab.getAttribute("aria-selected")).toBe("true");
    expect(importTab.getAttribute("aria-selected")).toBe("false");
    expect(settingsTab.classList.contains("tabs__tab--active")).toBe(true);
  });
});

describe("lint config UI round trip", () => {
  it("applyLintConfigToUI followed by lintConfigFromUI reproduces the config", () => {
    const config: LintConfig = {
      semicolon: true,
      indentation: "space",
      indentation_width: 8,
      cyclomatic_complexity_warning: 5,
      cyclomatic_complexity_error: 15,
      rules: { ...DEFAULT_RULES, forbidden_functions: false, indentation: false },
    };

    applyLintConfigToUI(config);
    expect(lintConfigFromUI()).toEqual(config);
  });

  it("applyLintConfigToUI enables the width field only for space indentation", () => {
    applyLintConfigToUI({ ...DEFAULT_LINT_CONFIG, indentation: "space" });
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(false);

    applyLintConfigToUI({ ...DEFAULT_LINT_CONFIG, indentation: "tab" });
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(true);
  });

  it("lintConfigFromUI clamps indentation width and complexity thresholds", () => {
    document.querySelector<HTMLInputElement>("#indentation-width")!.value = "100";
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-warning")!.value = "-5";
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-error")!.value = "-5";

    const config = lintConfigFromUI();
    expect(config.indentation_width).toBe(16);
    expect(config.cyclomatic_complexity_warning).toBe(1);
    expect(config.cyclomatic_complexity_error).toBe(1);
  });

  it("handleLintConfigChanged persists the config only once a project dir is known", async () => {
    handleLintConfigChanged();
    expect(invokeMock).not.toHaveBeenCalled();

    invokeImplFor({ load_lint_config: () => DEFAULT_LINT_CONFIG });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLSelectElement>("#semicolon-style")!.value = "require";
    handleLintConfigChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_lint_config", {
      dir: "/proj",
      config: expect.objectContaining({ semicolon: true }),
    });
  });
});

describe("loadLintConfig / saveLintConfig", () => {
  it("loadLintConfig returns the backend's config on success", async () => {
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({ load_lint_config: () => custom });

    await expect(loadLintConfig("/proj")).resolves.toEqual(custom);
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/proj" });
  });

  it("loadLintConfig falls back to the default config on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadLintConfig("/proj")).resolves.toEqual(DEFAULT_LINT_CONFIG);
  });

  it("saveLintConfig swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveLintConfig("/proj", DEFAULT_LINT_CONFIG)).resolves.toBeUndefined();
  });
});

describe("useProjectDir", () => {
  it("loads the config, applies it to the UI, and remembers the directory", async () => {
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true, indentation: "space" };
    invokeImplFor({ load_lint_config: () => custom });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("require");
    expect(document.querySelector<HTMLSelectElement>("#indentation-style")!.value).toBe("spaces");
    expect(lastProjectDir()).toBe("/my/project");
  });
});

describe("parsePscFiles / repairPscFile", () => {
  it("reports a successful parse with its lint findings", async () => {
    invokeImplFor({
      parse_psc_file: () => ({ name: "MyScript" }),
      lint_psc_file: () => [{ line: 2, column: 1, message: "Line contains trailing whitespace" }],
    });

    const [outcome] = await parsePscFiles(["/scripts/MyScript.psc"]);
    expect(outcome).toEqual({
      path: "/scripts/MyScript.psc",
      ok: true,
      detail: 'parsed as "MyScript"',
      findings: [{ line: 2, column: 1, message: "Line contains trailing whitespace" }],
    });
  });

  it("reports a failed parse with no findings", async () => {
    invokeMock.mockRejectedValue(new Error("syntax error"));

    const [outcome] = await parsePscFiles(["/scripts/Broken.psc"]);
    expect(outcome.ok).toBe(false);
    expect(outcome.detail).toContain("syntax error");
    expect(outcome.findings).toEqual([]);
  });

  it("repairPscFile forwards to the repair_psc_file command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_file: () => remaining });

    await expect(repairPscFile("/scripts/MyScript.psc")).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
    });
  });
});

describe("buildPscResultItem / renderPscResults", () => {
  function outcome(overrides: Partial<PscParseOutcome> = {}): PscParseOutcome {
    return { path: "/a.psc", ok: true, detail: 'parsed as "A"', findings: [], ...overrides };
  }

  it("skips a clean, successfully parsed file", () => {
    expect(buildPscResultItem(outcome())).toBeNull();
  });

  it("always shows a file that failed to parse, even with no findings", () => {
    const item = buildPscResultItem(outcome({ ok: false, detail: "boom" }));
    expect(item).not.toBeNull();
    expect(item!.classList.contains("psc-result__item--error")).toBe(true);
    expect(item!.textContent).toContain("boom");
  });

  it("shows a fix button only when findings are auto-fixable", () => {
    const fixable = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "Line contains trailing whitespace" }] }),
    );
    expect(fixable!.querySelector(".psc-result__fix-button")).not.toBeNull();

    const unfixable = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "[error] forbidden function used" }] }),
    );
    expect(unfixable!.querySelector(".psc-result__fix-button")).toBeNull();
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

  it("renderPscResults hides the panel entirely for an empty outcome list", () => {
    renderPscResults([{ findings: [{ line: 1, column: 1, message: "[error] x" }], ok: true, path: "/a.psc", detail: "" }]);
    renderPscResults([]);
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(true);
  });

  it("renderPscResults switches to the lint tab and lists visible findings", () => {
    switchTab("import");
    renderPscResults([
      outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] }),
      outcome({ path: "/b.psc" }),
    ]);

    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  });

  it("renderPscResults respects the active severity filters", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    renderPscResults([outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] })]);

    // The only finding is filtered out, and the file itself parsed cleanly,
    // so it should be skipped entirely.
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(0);
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
});

describe("showError / clearError / showResult", () => {
  it("showError displays the message, hides results, and switches to the import tab", () => {
    switchTab("lint");
    showError("bad file");

    expect(document.querySelector("#drop-zone-error")!.textContent).toBe("bad file");
    expect(document.querySelector("#achlist-result")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-import")!.hidden).toBe(false);
  });

  it("clearError empties the error message", () => {
    showError("bad file");
    clearError();
    expect(document.querySelector("#drop-zone-error")!.textContent).toBe("");
  });

  it("showResult lists the entries and switches to the files tab", () => {
    showResult("/a.achlist", ["one.psc", "two.psc"]);

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /a.achlist");
    expect(document.querySelectorAll("#achlist-result-list > li")).toHaveLength(2);
    expect(document.querySelector("#achlist-result")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelector<HTMLElement>("#panel-files")!.hidden).toBe(false);
  });
});

describe("handleDroppedPaths", () => {
  it("rejects a drop with no .achlist file", async () => {
    await handleDroppedPaths(["/scripts/A.psc"]);
    expect(document.querySelector("#drop-zone-error")!.textContent).toContain(".achlist");
  });

  it("parses the achlist, loads project config, and lints each .psc entry", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "readme.txt"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    await handleDroppedPaths(["/proj/list.achlist"]);

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/list.achlist");
    expect(lastProjectDir()).toBe("/proj");
    // readme.txt isn't a .psc file, so only A.psc should have been linted.
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "A.psc" });
    expect(invokeMock).not.toHaveBeenCalledWith("parse_psc_file", { path: "readme.txt" });
  });

  it("shows an error when the achlist itself fails to parse", async () => {
    invokeMock.mockRejectedValue(new Error("bad json"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleDroppedPaths(["/proj/list.achlist"]);

    expect(document.querySelector("#drop-zone-error")!.textContent).toContain("Failed to read");
  });
});

describe("openCodeViewer", () => {
  it("loads and highlights the source, opening the dialog", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")' });

    await openCodeViewer("/a.psc", []);

    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    expect(dialog.hasAttribute("open")).toBe(true);
    expect(document.querySelector("#code-viewer-title")!.textContent).toBe("/a.psc");
    expect(document.querySelector("#code-viewer-body table")).not.toBeNull();
    expect(document.querySelectorAll("#code-viewer-body tr")).toHaveLength(1);
  });

  it("marks a line's severity from its highest-severity finding", async () => {
    invokeImplFor({ read_psc_file: () => "line one\nline two\n" });

    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] risky" },
      { line: 1, column: 1, message: "[error] bad" },
    ]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.classList.contains("code-viewer__line--error")).toBe(true);
  });

  it("flags a line whose finding has no severity prefix", async () => {
    invokeImplFor({ read_psc_file: () => "line one\n" });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "Line contains trailing whitespace" }]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.classList.contains("code-viewer__line--flagged")).toBe(true);
  });

  it("shows a failure message when the file can't be read", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));

    await openCodeViewer("/a.psc", []);

    expect(document.querySelector("#code-viewer-body")!.textContent).toContain("permission denied");
  });
});

describe("toggleCodeViewerFullscreen", () => {
  it("toggles the fullscreen class and button state", () => {
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fullscreen")!;

    toggleCodeViewerFullscreen();
    expect(dialog.classList.contains("code-viewer--fullscreen")).toBe(true);
    expect(button.getAttribute("aria-pressed")).toBe("true");

    toggleCodeViewerFullscreen();
    expect(dialog.classList.contains("code-viewer--fullscreen")).toBe(false);
    expect(button.getAttribute("aria-pressed")).toBe("false");
  });
});

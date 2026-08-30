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
  applyAutocompleteSelection,
  applyLintConfigToUI,
  applyTheme,
  applyScriptRootsToUI,
  buildPscResultItem,
  cancelCodeViewerEditMode,
  clearError,
  dirnameOf,
  enterCodeViewerEditMode,
  escapeAttr,
  handleAutocompleteKeydown,
  handleCompileClick,
  handleCompilerPathChanged,
  handleDroppedPaths,
  handleFixClick,
  handleLintConfigChanged,
  handleScriptRootsChanged,
  hasFixableFindings,
  hideAutocomplete,
  isAchlistPath,
  isCodeViewerEditDirty,
  isPscPath,
  lastProjectDir,
  levelOf,
  lintConfigFromUI,
  listScriptMembers,
  loadAppVersion,
  loadCompilerPath,
  loadLintConfig,
  loadStoredTheme,
  loadScriptRoots,
  openCodeViewer,
  parsePscFiles,
  relativePath,
  rememberProjectDir,
  renderPscResults,
  repairPscFile,
  requestCloseCodeViewer,
  saveCodeViewerEdits,
  saveCompilerPath,
  saveLintConfig,
  saveScriptRoots,
  scriptRootsFromUI,
  severityOf,
  showError,
  showResult,
  storeTheme,
  switchTab,
  toggleCodeViewerFullscreen,
  updateAutocomplete,
  useProjectDir,
  type Diagnostic,
  type LintConfig,
  type PscParseOutcome,
} from "./main";

function invokeImplFor(handlers: Record<string, (args: unknown) => unknown>) {
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

  it("relativePath strips a matching base prefix, for either slash style", () => {
    expect(relativePath("/proj/scripts/A.psc", "/proj")).toBe("scripts/A.psc");
    expect(relativePath("C:\\proj\\scripts\\A.psc", "C:\\proj")).toBe("scripts\\A.psc");
  });

  it("relativePath falls back to the absolute path when base is unknown or unrelated", () => {
    expect(relativePath("/a.psc", null)).toBe("/a.psc");
    expect(relativePath("/elsewhere/A.psc", "/proj")).toBe("/elsewhere/A.psc");
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
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace" }]),
    ).toBe(true);
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

describe("theme", () => {
  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("applyTheme sets data-theme for light/dark and clears it for system", () => {
    applyTheme("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    applyTheme("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    applyTheme("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("loadStoredTheme defaults to system when nothing is stored", () => {
    expect(loadStoredTheme()).toBe("system");
  });

  it("round-trips a stored theme through localStorage", () => {
    storeTheme("dark");
    expect(loadStoredTheme()).toBe("dark");
  });

  it("loadStoredTheme falls back to system for an unrecognized value", () => {
    localStorage.setItem("papyrus-lint:theme", "purple");
    expect(loadStoredTheme()).toBe("system");
  });

  it("loadStoredTheme tolerates a broken localStorage", () => {
    const getItemSpy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(loadStoredTheme()).toBe("system");
    getItemSpy.mockRestore();
  });

  it("storeTheme tolerates a broken localStorage", () => {
    const setItemSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => storeTheme("dark")).not.toThrow();
    setItemSpy.mockRestore();
  });

  it("initializes the theme select from storage and applies the theme on startup", () => {
    localStorage.setItem("papyrus-lint:theme", "dark");
    mountFixture();
    expect(document.querySelector<HTMLSelectElement>("#theme-select")!.value).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("persists and applies the theme when the select changes", () => {
    mountFixture();
    const select = document.querySelector<HTMLSelectElement>("#theme-select")!;
    select.value = "light";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("papyrus-lint:theme")).toBe("light");
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
      identifier_casing: "snake_case",
      cyclomatic_complexity_warning: 5,
      cyclomatic_complexity_error: 15,
      type_casing: "camelCase",
      named_arguments: "always",
      fail_on_warning: true,
      fail_on_info: true,
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

  it("applies and reads back identifier casing, named arguments, and fail-on-level settings", () => {
    applyLintConfigToUI({
      ...DEFAULT_LINT_CONFIG,
      identifier_casing: "CONSTANT_CASE",
      named_arguments: "instead_of_defaults",
      fail_on_warning: true,
      fail_on_info: true,
    });

    expect(document.querySelector<HTMLSelectElement>("#identifier-casing-style")!.value).toBe(
      "CONSTANT_CASE",
    );
    expect(document.querySelector<HTMLSelectElement>("#named-arguments-style")!.value).toBe(
      "instead_of_defaults",
    );
    expect(document.querySelector<HTMLInputElement>("#fail-on-warning")!.checked).toBe(true);
    expect(document.querySelector<HTMLInputElement>("#fail-on-info")!.checked).toBe(true);

    const config = lintConfigFromUI();
    expect(config.identifier_casing).toBe("CONSTANT_CASE");
    expect(config.named_arguments).toBe("instead_of_defaults");
    expect(config.fail_on_warning).toBe(true);
    expect(config.fail_on_info).toBe(true);
  });

  it("handleLintConfigChanged persists the config only once a project dir is known", async () => {
    handleLintConfigChanged();
    expect(invokeMock).not.toHaveBeenCalledWith("save_lint_config", expect.anything());

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

  it("handleCompilerPathChanged persists the path once a project dir is known", async () => {
    invokeImplFor({ load_lint_config: () => DEFAULT_LINT_CONFIG, load_compiler_path: () => null });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLInputElement>("#compiler-path")!.value = "C:\\Tools\\PapyrusCompiler.exe";
    handleCompilerPathChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_compiler_path", {
      dir: "/proj",
      path: "C:\\Tools\\PapyrusCompiler.exe",
    });
  });

  it("handleScriptRootsChanged persists the roots once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => [],
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLTextAreaElement>("#script-roots")!.value =
      "../SharedScripts\n\n  /abs/OtherScripts  \n";
    handleScriptRootsChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_script_roots", {
      dir: "/proj",
      roots: ["../SharedScripts", "/abs/OtherScripts"],
    });
  });
});

describe("scriptRootsFromUI / applyScriptRootsToUI", () => {
  it("scriptRootsFromUI splits non-blank lines and trims whitespace", () => {
    document.querySelector<HTMLTextAreaElement>("#script-roots")!.value =
      "  ../SharedScripts  \n\n/abs/OtherScripts\n";

    expect(scriptRootsFromUI()).toEqual(["../SharedScripts", "/abs/OtherScripts"]);
  });

  it("scriptRootsFromUI returns an empty array for a blank textarea", () => {
    document.querySelector<HTMLTextAreaElement>("#script-roots")!.value = "   \n  \n";

    expect(scriptRootsFromUI()).toEqual([]);
  });

  it("applyScriptRootsToUI joins roots with newlines", () => {
    applyScriptRootsToUI(["../SharedScripts", "/abs/OtherScripts"]);

    expect(document.querySelector<HTMLTextAreaElement>("#script-roots")!.value).toBe(
      "../SharedScripts\n/abs/OtherScripts",
    );
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
    invokeImplFor({
      load_lint_config: () => custom,
      load_compiler_path: () => null,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("require");
    expect(document.querySelector<HTMLSelectElement>("#indentation-style")!.value).toBe("spaces");
    expect(lastProjectDir()).toBe("/my/project");
  });

  it("populates the compiler path input from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe",
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLInputElement>("#compiler-path")!.value).toBe(
      "C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe",
    );
  });

  it("populates the script roots textarea from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => ["../SharedScripts", "/abs/OtherScripts"],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLTextAreaElement>("#script-roots")!.value).toBe(
      "../SharedScripts\n/abs/OtherScripts",
    );
  });
});

describe("loadAppVersion", () => {
  it("returns the backend's reported version", async () => {
    invokeImplFor({ get_app_version: () => "1.2.3" });

    await expect(loadAppVersion()).resolves.toBe("1.2.3");
    expect(invokeMock).toHaveBeenCalledWith("get_app_version");
  });

  it("returns an empty string on failure", async () => {
    invokeMock.mockRejectedValue(new Error("command not found"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadAppVersion()).resolves.toBe("");
  });
});

describe("loadCompilerPath / saveCompilerPath", () => {
  it("loadCompilerPath returns the backend's resolved path", async () => {
    invokeImplFor({ load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe" });

    await expect(loadCompilerPath("/proj")).resolves.toBe("C:\\Tools\\PapyrusCompiler.exe");
    expect(invokeMock).toHaveBeenCalledWith("load_compiler_path", { dir: "/proj" });
  });

  it("loadCompilerPath returns an empty string when the backend has none", async () => {
    invokeImplFor({ load_compiler_path: () => null });

    await expect(loadCompilerPath("/proj")).resolves.toBe("");
  });

  it("loadCompilerPath returns an empty string on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadCompilerPath("/proj")).resolves.toBe("");
  });

  it("saveCompilerPath swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveCompilerPath("/proj", "C:\\Tools\\PapyrusCompiler.exe")).resolves.toBeUndefined();
  });
});

describe("loadScriptRoots / saveScriptRoots", () => {
  it("loadScriptRoots returns the backend's configured roots", async () => {
    invokeImplFor({ load_script_roots: () => ["../SharedScripts", "/abs/OtherScripts"] });

    await expect(loadScriptRoots("/proj")).resolves.toEqual(["../SharedScripts", "/abs/OtherScripts"]);
    expect(invokeMock).toHaveBeenCalledWith("load_script_roots", { dir: "/proj" });
  });

  it("loadScriptRoots returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadScriptRoots("/proj")).resolves.toEqual([]);
  });

  it("saveScriptRoots forwards the roots to the backend", async () => {
    invokeImplFor({ save_script_roots: () => undefined });

    await expect(saveScriptRoots("/proj", ["../SharedScripts"])).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_script_roots", {
      dir: "/proj",
      roots: ["../SharedScripts"],
    });
  });

  it("saveScriptRoots swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveScriptRoots("/proj", ["../SharedScripts"])).resolves.toBeUndefined();
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
      additionalRoots: expect.anything(),
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
      outcome({ findings: [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace" }] }),
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
    expect(document.querySelector("#code-viewer-view table")).not.toBeNull();
    expect(document.querySelectorAll("#code-viewer-view tr")).toHaveLength(1);
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

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "No recognized level prefix here" }]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.classList.contains("code-viewer__line--flagged")).toBe(true);
  });

  it("shows a failure message when the file can't be read", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));

    await openCodeViewer("/a.psc", []);

    expect(document.querySelector("#code-viewer-view")!.textContent).toContain("permission denied");
  });
});

describe("code viewer edit mode", () => {
  async function openWithSource(source: string) {
    invokeImplFor({ read_psc_file: () => source });
    await openCodeViewer("/a.psc", []);
  }

  function textarea() {
    return document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
  }

  function highlightCode() {
    return document.querySelector("#code-viewer-editor-highlight code")!;
  }

  function panelHidden(id: string) {
    return document.querySelector<HTMLElement>(id)!.hidden;
  }

  describe("enterCodeViewerEditMode", () => {
    it("loads the source into the textarea, highlights it, and switches to edit mode", async () => {
      await openWithSource('Debug.Trace("hi")\n');

      enterCodeViewerEditMode();

      expect(textarea().value).toBe('Debug.Trace("hi")\n');
      expect(highlightCode().innerHTML).toContain("Debug");
      expect(panelHidden("#code-viewer-view")).toBe(true);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
      expect(panelHidden("#code-viewer-edit")).toBe(true);
      expect(panelHidden("#code-viewer-save")).toBe(false);
      expect(panelHidden("#code-viewer-cancel")).toBe(false);
    });

    it("does nothing when the code viewer hasn't finished loading", async () => {
      // A failed read leaves codeViewerState null (openCodeViewer resets it
      // to null up front and only repopulates it after a successful read).
      invokeMock.mockRejectedValue(new Error("permission denied"));
      await openCodeViewer("/a.psc", []);

      enterCodeViewerEditMode();

      expect(panelHidden("#code-viewer-editor")).toBe(true);
    });
  });

  describe("isCodeViewerEditDirty", () => {
    it("is false right after entering edit mode and true once the textarea changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();

      expect(isCodeViewerEditDirty()).toBe(false);

      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));

      expect(isCodeViewerEditDirty()).toBe(true);
      // The "input" listener re-highlights the edited text as it changes.
      expect(highlightCode().innerHTML).toContain("2");
    });

    it("is false in view mode even with a stale textarea value", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(true);
      cancelCodeViewerEditMode();

      expect(isCodeViewerEditDirty()).toBe(false);
    });
  });

  describe("cancelCodeViewerEditMode", () => {
    it("returns to view mode without confirming when there are no unsaved changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const confirmSpy = vi.spyOn(window, "confirm");

      cancelCodeViewerEditMode();

      expect(confirmSpy).not.toHaveBeenCalled();
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("stays in edit mode when the user declines to discard unsaved changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(false);

      cancelCodeViewerEditMode();

      expect(panelHidden("#code-viewer-editor")).toBe(false);
    });

    it("discards unsaved changes and returns to view mode when the user confirms", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(true);

      cancelCodeViewerEditMode();

      expect(panelHidden("#code-viewer-view")).toBe(false);
    });
  });

  describe("saveCodeViewerEdits", () => {
    it("writes the file, re-lints it, and returns to view mode", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] changed" }],
      });

      await saveCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", { path: "/a.psc", contents: "Int x = 2\n" });
      expect(panelHidden("#code-viewer-view")).toBe(false);
      expect(isCodeViewerEditDirty()).toBe(false);
    });

    it("updates the matching lint results entry when one is open", async () => {
      // activeSeverities is module state that outlives mountFixture(), so an
      // earlier test unchecking a severity filter would otherwise leak in.
      const errorFilter = document.querySelector<HTMLInputElement>("#filter-error")!;
      errorFilter.checked = true;
      errorFilter.dispatchEvent(new Event("change"));

      invokeImplFor({
        parse_achlist_file: () => ["A.psc"],
        load_lint_config: () => DEFAULT_LINT_CONFIG,
        load_compiler_path: () => null,
        load_script_roots: () => [],
        parse_psc_file: () => ({ name: "A" }),
        lint_psc_file: () => [],
      });
      await handleDroppedPaths(["/proj/list.achlist"]);
      // The dropped file's outcome is keyed by the same path handed to
      // parse_psc_file above, so the code viewer must be opened on it too.
      invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
      await openCodeViewer("A.psc", []);
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [{ line: 1, column: 1, message: "[error] changed" }],
      });

      await saveCodeViewerEdits();

      expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    });

    it("shows a failure and stays in edit mode when writing fails", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeMock.mockRejectedValue(new Error("disk full"));
      vi.spyOn(console, "error").mockImplementation(() => {});
      const saveButton = document.querySelector<HTMLButtonElement>("#code-viewer-save")!;

      await saveCodeViewerEdits();

      expect(saveButton.textContent).toBe("Save failed");
      expect(saveButton.disabled).toBe(false);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
    });
  });

  describe("autocompletion", () => {
    const SELF_MEMBER_SCRIPT = "ScriptName Example\n\nFunction Run()\n    self.\nEndFunction\n";

    function autocompleteEl() {
      return document.querySelector<HTMLElement>("#code-viewer-autocomplete")!;
    }

    // Enters edit mode on SELF_MEMBER_SCRIPT and places the caret right
    // after "self.", the spot autocompletion should trigger from.
    async function openWithCursorAfterSelfDot() {
      await openWithSource(SELF_MEMBER_SCRIPT);
      enterCodeViewerEditMode();
      const field = textarea();
      const cursor = field.value.indexOf("self.") + "self.".length;
      field.setSelectionRange(cursor, cursor);
      return field;
    }

    it("does nothing while not in edit mode", async () => {
      await openWithSource(SELF_MEMBER_SCRIPT);

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
    });

    it("queries list_script_members for the receiver's declared type and shows the results", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: { name: "String", is_array: false },
            is_global: false,
            is_native: true,
            is_event: false,
          },
          { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false } },
        ],
      });

      await updateAutocomplete();

      expect(invokeMock).toHaveBeenCalledWith("list_script_members", expect.objectContaining({ typeName: "Example" }));
      expect(autocompleteEl().hidden).toBe(false);
      expect(autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item")).toHaveLength(2);
    });

    it("hides the dropdown when the cursor isn't right after a member access", async () => {
      await openWithSource("ScriptName Example\n\nFunction Run()\n    Int i = 0\nEndFunction\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(field.value.length, field.value.length);

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
    });

    it("applyAutocompleteSelection splices the chosen member's insertion text in place of the typed prefix", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: null,
            is_global: false,
            is_native: false,
            is_event: false,
          },
        ],
      });
      await updateAutocomplete();

      applyAutocompleteSelection(0);

      expect(field.value).toBe("ScriptName Example\n\nFunction Run()\n    self.GetName(\nEndFunction\n");
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("navigates with the arrow keys and accepts the highlighted entry on Enter", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } },
          { kind: "property", name: "BProp", type_name: { name: "Int", is_array: false } },
        ],
      });
      await updateAutocomplete();

      const downEvent = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
      handleAutocompleteKeydown(downEvent);
      expect(downEvent.defaultPrevented).toBe(true);
      expect(autocompleteEl().querySelector(".code-viewer__autocomplete-item--active")?.textContent).toContain(
        "BProp",
      );

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Enter", cancelable: true }));

      expect(field.value).toContain("self.BProp");
    });

    it("dismisses the dropdown on Escape without touching the textarea", async () => {
      const field = await openWithCursorAfterSelfDot();
      const beforeEscape = field.value;
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));

      expect(autocompleteEl().hidden).toBe(true);
      expect(field.value).toBe(beforeEscape);
    });

    it("hideAutocomplete clears any pending dropdown", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      hideAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item")).toHaveLength(0);
    });

    it("listScriptMembers logs and returns an empty list when the backend call fails", async () => {
      invokeMock.mockRejectedValue(new Error("lookup failed"));
      vi.spyOn(console, "error").mockImplementation(() => {});

      await expect(listScriptMembers("Example")).resolves.toEqual([]);
    });
  });
});

describe("requestCloseCodeViewer", () => {
  it("closes the dialog when there are no unsaved changes", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
    await openCodeViewer("/a.psc", []);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;

    requestCloseCodeViewer();

    expect(dialog.hasAttribute("open")).toBe(false);
  });

  it("keeps the dialog open when the user declines to discard unsaved edit-mode changes", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
    await openCodeViewer("/a.psc", []);
    enterCodeViewerEditMode();
    document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!.value = "Int x = 2\n";
    vi.spyOn(window, "confirm").mockReturnValue(false);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;

    requestCloseCodeViewer();

    expect(dialog.hasAttribute("open")).toBe(true);
  });

  it("closes the dialog when the user confirms discarding unsaved edit-mode changes", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
    await openCodeViewer("/a.psc", []);
    enterCodeViewerEditMode();
    document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!.value = "Int x = 2\n";
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;

    requestCloseCodeViewer();

    expect(dialog.hasAttribute("open")).toBe(false);
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

describe("wired DOM interactions", () => {
  it("drives the tab and settings controls through their registered listeners", async () => {
    invokeMock.mockResolvedValue(undefined);

    document.querySelector<HTMLButtonElement>("#tab-settings")!.click();
    expect(document.querySelector<HTMLElement>("#panel-settings")!.hidden).toBe(false);

    const indentation = document.querySelector<HTMLSelectElement>("#indentation-style")!;
    indentation.value = "spaces";
    indentation.dispatchEvent(new Event("change"));
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(false);

    for (const selector of [
      "#compiler-path",
      "#semicolon-style",
      "#indentation-width",
      "#identifier-casing-style",
      "#named-arguments-style",
      "#cyclomatic-complexity-warning",
      "#cyclomatic-complexity-error",
      "#fail-on-warning",
      "#fail-on-info",
      "#rule-trailing_whitespace",
      "#rule-property_sorting",
    ]) {
      document.querySelector<HTMLElement>(selector)!.dispatchEvent(new Event("change"));
    }
    await Promise.resolve();
  });

  it("handles all drag/drop event variants", async () => {
    const registration = onDragDropEventMock.mock.calls[onDragDropEventMock.mock.calls.length - 1];
    expect(registration).toBeDefined();
    const listener = registration![0] as (event: { payload: { type: string; paths: string[] } }) => void;
    const dropZone = document.querySelector<HTMLElement>("#drop-zone")!;

    listener({ payload: { type: "over", paths: [] } });
    expect(dropZone.classList.contains("drop-zone--active")).toBe(true);
    listener({ payload: { type: "cancel", paths: [] } });
    expect(dropZone.classList.contains("drop-zone--active")).toBe(false);

    invokeImplFor({
      parse_achlist_file: () => [],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => [],
    });
    listener({ payload: { type: "drop", paths: ["/proj/list.achlist"] } });
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith("parse_achlist_file", { path: "/proj/list.achlist" }));
  });

  it("wires result action buttons and finding links", async () => {
    const outcome: PscParseOutcome = {
      path: "/a.psc",
      ok: true,
      detail: "parsed",
      findings: [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace" }],
    };
    const item = buildPscResultItem(outcome)!;
    document.body.append(item);

    invokeImplFor({
      read_psc_file: () => "Int x = 1 ",
      repair_psc_file: () => [],
      compile_psc_file: () => ({ success: true, stdout: "ok", stderr: "" }),
    });
    item.querySelector<HTMLButtonElement>(".psc-result__view-button")!.click();
    item.querySelector<HTMLElement>(".psc-result__finding")!.click();
    item.querySelector<HTMLButtonElement>(".psc-result__fix-button")!.click();
    item.querySelector<HTMLButtonElement>(".psc-result__compile-button")!.click();

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.anything()));
    await vi.waitFor(() => expect(item.querySelector(".psc-result__compile-output")!.textContent).toBe("ok"));
  });

  it("wires code viewer buttons, input, scrolling, backdrop, cancel, and close", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
    await openCodeViewer("/a.psc", []);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const textarea = document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
    const highlight = document.querySelector<HTMLElement>("#code-viewer-editor-highlight")!;

    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();
    textarea.value = "Int x = 2\n";
    textarea.dispatchEvent(new Event("input"));
    textarea.scrollTop = 12;
    textarea.scrollLeft = 7;
    textarea.dispatchEvent(new Event("scroll"));
    expect(highlight.scrollTop).toBe(12);
    expect(highlight.scrollLeft).toBe(7);

    vi.spyOn(window, "confirm").mockReturnValue(false);
    const cancelEvent = new Event("cancel", { cancelable: true });
    dialog.dispatchEvent(cancelEvent);
    expect(cancelEvent.defaultPrevented).toBe(true);
    dialog.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(dialog.open).toBe(true);

    vi.mocked(window.confirm).mockReturnValue(true);
    document.querySelector<HTMLButtonElement>("#code-viewer-cancel")!.click();
    document.querySelector<HTMLButtonElement>("#code-viewer-fullscreen")!.click();
    dialog.dispatchEvent(new Event("close"));
    expect(dialog.classList.contains("code-viewer--fullscreen")).toBe(false);
    document.querySelector<HTMLButtonElement>("#code-viewer-close")!.click();
  });
});

describe("remaining failure and defensive paths", () => {
  it("lintPscFile logs backend failures and returns no diagnostics", async () => {
    invokeMock.mockRejectedValue(new Error("lint failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(import("./main").then(({ lintPscFile }) => lintPscFile("/a.psc"))).resolves.toEqual([]);
  });

  it("rememberProjectDir tolerates unavailable storage", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => rememberProjectDir("/proj")).not.toThrow();
  });

  it("reports repair failures without rejecting", async () => {
    invokeMock.mockRejectedValue(new Error("repair failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const button = document.createElement("button");
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "parsed", findings: [] };
    await expect(handleFixClick("/a.psc", outcome, button)).resolves.toBeUndefined();
    expect(button.disabled).toBe(true);
  });

  it("restores the remembered project and displays the app version on startup", async () => {
    localStorage.setItem("papyrus-lint:last-project-dir", "/remembered");
    invokeImplFor({
      get_app_version: () => "1.2.3",
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => [],
    });
    const version = document.createElement("span");
    version.id = "app-version";
    document.body.append(version);

    document.dispatchEvent(new Event("DOMContentLoaded", { bubbles: true }));
    await vi.waitFor(() => expect(version.textContent).toBe("v1.2.3"));
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/remembered" });
  });

  it("resets a failed save label after the timeout", async () => {
    vi.useFakeTimers();
    invokeImplFor({ read_psc_file: () => "old" });
    await openCodeViewer("/a.psc", []);
    enterCodeViewerEditMode();
    document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!.value = "new";
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await saveCodeViewerEdits();
    vi.runAllTimers();
    expect(document.querySelector<HTMLButtonElement>("#code-viewer-save")!.textContent).toBe("Save");
    vi.useRealTimers();
  });
});

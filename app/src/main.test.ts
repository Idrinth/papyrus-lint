import { afterEach, describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import {
  confirmDetectedConfig,
  invokeImplFor,
  loadProjectConfigConfirmed,
} from "./test/harness";
import { mountFixture } from "./test/fixture";
import {
  DEFAULT_LINT_CONFIG,
  DEFAULT_RULES,
  addDisableCommentToPscLine,
  applyLintConfigToUI,
  applyProjectInfoToUI,
  applyRuleTags,
  applyTheme,
  applyScriptRootsToUI,
  applyLookupScriptRootsToUI,
  buildPscResultItem,
  clearError,
  configPathOverride,
  dirnameOf,
  enterCodeViewerEditMode,
  escapeAttr,
  findCandidatePairRoot,
  handleCompileCheckChanged,
  handleCompilerPathChanged,
  handleConfigPathOverrideChanged,
  handleDroppedPaths,
  handleFixClick,
  handleLintConfigChanged,
  handleScriptRootsChanged,
  handleLookupScriptRootsChanged,
  hasFixableFindings,
  hideLintProgress,
  isAchlistPath,
  isFixableFinding,
  isPscPath,
  levelOf,
  lintConfigFromUI,
  loadAppVersion,
  loadCompileCheck,
  loadCompilerPath,
  loadLintConfig,
  loadLintConfigFromPath,
  loadProjectConfig,
  loadProjectInfo,
  loadRuleTags,
  loadStoredTheme,
  loadScriptRoots,
  loadLookupScriptRoots,
  openCodeViewer,
  parsePscFiles,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPscPath,
  relativePath,
  relintCurrentFiles,
  previewRepairPscFile,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  saveCodeViewerEdits,
  saveCompileCheck,
  saveCompilerPath,
  saveLintConfig,
  saveLintConfigToPath,
  saveScriptRoots,
  saveLookupScriptRoots,
  scheduleHideLintProgress,
  scriptRootsFromUI,
  lookupScriptRootsFromUI,
  scriptRootsForAchlist,
  setSettingsLocked,
  severityOf,
  showError,
  showLintProgress,
  showResult,
  storeTheme,
  switchTab,
  tagsForFinding,
  updateLintProgress,
  useProjectDir,
  type Diagnostic,
  type LintConfig,
  type PscParseOutcome,
  type RuleTagsInfo,
} from "./main";

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

  it("severityOf falls back to 'error' when there is no level prefix", () => {
    expect(severityOf("[error] boom")).toBe("error");
    expect(severityOf("no prefix here")).toBe("error");
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
      min_wait_interval: 0.25,
      magic_numbers: "strict",
      fail_on_warning: true,
      fail_on_info: true,
      bool_like_int: false,
      assume_auto_properties_filled: true,
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

  it("lintConfigFromUI never lets the error threshold fall below the warning one", () => {
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-warning")!.value = "30";
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-error")!.value = "20";

    const config = lintConfigFromUI();
    expect(config.cyclomatic_complexity_warning).toBe(30);
    expect(config.cyclomatic_complexity_error).toBe(30);
  });

  it("applies and reads back identifier casing, named arguments, and fail-on-level settings", () => {
    applyLintConfigToUI({
      ...DEFAULT_LINT_CONFIG,
      identifier_casing: "CONSTANT_CASE",
      named_arguments: "instead_of_defaults",
      fail_on_warning: true,
      fail_on_info: true,
      bool_like_int: false,
      assume_auto_properties_filled: true,
    });

    expect(document.querySelector<HTMLSelectElement>("#identifier-casing-style")!.value).toBe(
      "CONSTANT_CASE",
    );
    expect(document.querySelector<HTMLSelectElement>("#named-arguments-style")!.value).toBe(
      "instead_of_defaults",
    );
    expect(document.querySelector<HTMLInputElement>("#fail-on-warning")!.checked).toBe(true);
    expect(document.querySelector<HTMLInputElement>("#fail-on-info")!.checked).toBe(true);
    expect(document.querySelector<HTMLInputElement>("#bool-like-int")!.checked).toBe(false);
    expect(
      document.querySelector<HTMLInputElement>("#assume-auto-properties-filled")!.checked,
    ).toBe(true);

    const config = lintConfigFromUI();
    expect(config.identifier_casing).toBe("CONSTANT_CASE");
    expect(config.named_arguments).toBe("instead_of_defaults");
    expect(config.fail_on_warning).toBe(true);
    expect(config.fail_on_info).toBe(true);
    expect(config.bool_like_int).toBe(false);
    expect(config.assume_auto_properties_filled).toBe(true);
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

  it("handleLintConfigChanged saves to the configuration file override instead, when one is set", async () => {
    invokeImplFor({ load_lint_config: () => DEFAULT_LINT_CONFIG });
    await useProjectDir("/proj");
    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "/profiles/strict.yaml";
    invokeMock.mockClear();

    document.querySelector<HTMLSelectElement>("#semicolon-style")!.value = "require";
    handleLintConfigChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_lint_config_to_path", {
      path: "/profiles/strict.yaml",
      config: expect.objectContaining({ semicolon: true }),
    });
    expect(invokeMock).not.toHaveBeenCalledWith("save_lint_config", expect.anything());
  });

  it("handleCompilerPathChanged persists the path once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
    });
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

  it("handleCompileCheckChanged persists the setting once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLInputElement>("#compile-check")!.checked = true;
    handleCompileCheckChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_compile_check", { dir: "/proj", enabled: true });
  });

  it("handleScriptRootsChanged persists the roots once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
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

  it("handleLookupScriptRootsChanged persists the roots once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_lookup_script_roots: () => [],
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value =
      "C:/Skyrim/Data/Scripts/Source\n\n  C:/Skyrim/Data/Source/Scripts  \n";
    handleLookupScriptRootsChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_lookup_script_roots", {
      dir: "/proj",
      roots: ["C:/Skyrim/Data/Scripts/Source", "C:/Skyrim/Data/Source/Scripts"],
    });
  });
});

describe("configuration file override", () => {
  it("configPathOverride reads and trims the settings input", () => {
    expect(configPathOverride()).toBe("");

    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "  /profiles/strict.yaml  ";
    expect(configPathOverride()).toBe("/profiles/strict.yaml");
  });

  it("handleConfigPathOverrideChanged reloads the current project's config from the new override path", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({
      load_lint_config_from_path: () => custom,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
    });
    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "/profiles/strict.yaml";
    handleConfigPathOverrideChanged();

    await vi.waitFor(() =>
      expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("require"),
    );
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", { path: "/profiles/strict.yaml" });
    expect(document.querySelector("#used-configuration-file")!.textContent).toBe("/profiles/strict.yaml");
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

describe("lookupScriptRootsFromUI / applyLookupScriptRootsToUI", () => {
  it("lookupScriptRootsFromUI splits non-blank lines and trims whitespace", () => {
    document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value =
      "  C:/Skyrim/Data/Scripts/Source  \n\nC:/Skyrim/Data/Source/Scripts\n";

    expect(lookupScriptRootsFromUI()).toEqual([
      "C:/Skyrim/Data/Scripts/Source",
      "C:/Skyrim/Data/Source/Scripts",
    ]);
  });

  it("lookupScriptRootsFromUI returns an empty array for a blank textarea", () => {
    document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value = "   \n  \n";

    expect(lookupScriptRootsFromUI()).toEqual([]);
  });

  it("applyLookupScriptRootsToUI joins roots with newlines", () => {
    applyLookupScriptRootsToUI(["C:/Skyrim/Data/Scripts/Source", "C:/Skyrim/Data/Source/Scripts"]);

    expect(document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value).toBe(
      "C:/Skyrim/Data/Scripts/Source\nC:/Skyrim/Data/Source/Scripts",
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

describe("loadLintConfigFromPath / saveLintConfigToPath", () => {
  it("loadLintConfigFromPath returns the backend's config on success", async () => {
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({ load_lint_config_from_path: () => custom });

    await expect(loadLintConfigFromPath("/profiles/strict.yaml")).resolves.toEqual(custom);
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", { path: "/profiles/strict.yaml" });
  });

  it("loadLintConfigFromPath falls back to the default config on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadLintConfigFromPath("/profiles/missing.yaml")).resolves.toEqual(DEFAULT_LINT_CONFIG);
  });

  it("saveLintConfigToPath persists the config to the given file", async () => {
    invokeImplFor({ save_lint_config_to_path: () => undefined });

    await saveLintConfigToPath("/profiles/strict.yaml", DEFAULT_LINT_CONFIG);

    expect(invokeMock).toHaveBeenCalledWith("save_lint_config_to_path", {
      path: "/profiles/strict.yaml",
      config: DEFAULT_LINT_CONFIG,
    });
  });

  it("saveLintConfigToPath swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveLintConfigToPath("/profiles/strict.yaml", DEFAULT_LINT_CONFIG)).resolves.toBeUndefined();
  });
});

describe("useProjectDir", () => {
  it("loads the config and applies it to the UI", async () => {
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true, indentation: "space" };
    invokeImplFor({
      load_lint_config: () => custom,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("require");
    expect(document.querySelector<HTMLSelectElement>("#indentation-style")!.value).toBe("spaces");
  });

  it("populates the compiler path input from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe",
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLInputElement>("#compiler-path")!.value).toBe(
      "C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe",
    );
  });

  it("populates the compile-check checkbox from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => true,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLInputElement>("#compile-check")!.checked).toBe(true);
  });

  it("populates the script roots textarea from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => ["../SharedScripts", "/abs/OtherScripts"],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLTextAreaElement>("#script-roots")!.value).toBe(
      "../SharedScripts\n/abs/OtherScripts",
    );
  });

  it("populates the lookup script roots textarea from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_lookup_script_roots: () => ["C:/Skyrim/Data/Scripts/Source", "C:/Skyrim/Data/Source/Scripts"],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value).toBe(
      "C:/Skyrim/Data/Scripts/Source\nC:/Skyrim/Data/Source/Scripts",
    );
  });

  it("shows detected script roots and the configuration file", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => [],
      load_project_info: () => ({
        detected_script_roots: ["/my/project/scripts/source", "/shared/scripts"],
        used_configuration_file: "/my/project/papyrus-lint.yml",
      }),
    });

    await useProjectDir("/my/project");

    expect(document.querySelector("#detected-script-roots")!.textContent).toBe(
      "/my/project/scripts/source\n/shared/scripts",
    );
    expect(document.querySelector("#used-configuration-file")!.textContent).toBe(
      "/my/project/papyrus-lint.yml",
    );
  });

  it("loads the config from the override path instead, when one is set", async () => {
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({
      load_lint_config_from_path: () => custom,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: "/my/project/papyrus-lint.yaml",
      }),
    });
    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "/profiles/strict.yaml";

    await useProjectDir("/my/project");

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", { path: "/profiles/strict.yaml" });
    expect(invokeMock).not.toHaveBeenCalledWith("load_lint_config", expect.anything());
    expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("require");
    expect(document.querySelector("#used-configuration-file")!.textContent).toBe("/profiles/strict.yaml");
  });

  it("never shows the config-selection dialog on its own - that's loadProjectConfig's job", async () => {
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(false);
  });
});

describe("setSettingsLocked", () => {
  it("disables the settings fieldset and shows the locked notice when locked", () => {
    setSettingsLocked(true);

    expect(document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!.disabled).toBe(true);
    expect(document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden).toBe(false);
  });

  it("enables the settings fieldset and hides the locked notice when unlocked", () => {
    setSettingsLocked(true);

    setSettingsLocked(false);

    expect(document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!.disabled).toBe(false);
    expect(document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden).toBe(true);
  });
});

describe("loadProjectConfig", () => {
  it("locks the Settings tab while the picker is open and unlocks it once resolved", async () => {
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() => expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true));
    expect(document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!.disabled).toBe(true);
    expect(document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden).toBe(false);

    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
    await pending;

    expect(document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!.disabled).toBe(false);
    expect(document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden).toBe(true);
  });

  it("uses auto-detection (no override) when Continue is picked", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: "/my/project/papyrus-lint.yaml",
      }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await loadProjectConfigConfirmed("/my/project");

    expect(configPathOverride()).toBe("");
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/my/project" });
  });

  it("applies the chosen preset before loading the project's config", async () => {
    const presets = [
      { id: "strict", label: "Strict", description: "Catches everything." },
      { id: "careful", label: "Careful", description: "The quietest option." },
    ];
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      list_config_presets: () => presets,
      apply_config_preset: () => undefined,
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(document.querySelectorAll("#config-picker-preset-list .config-picker__preset-option").length).toBe(2),
    );
    document
      .querySelectorAll<HTMLButtonElement>("#config-picker-preset-list .config-picker__preset-option")[1]
      .click();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("apply_config_preset", { dir: "/my/project", preset: "careful" });
  });

  it("passes a custom preset id to the backend without normalizing it", async () => {
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      list_config_presets: () => [
        { id: "Team Conventions", label: "Team Conventions", description: "A custom preset." },
      ],
      apply_config_preset: () => undefined,
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker-preset-list .config-picker__preset-option")).not.toBeNull(),
    );
    document
      .querySelector<HTMLButtonElement>("#config-picker-preset-list .config-picker__preset-option")!
      .click();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("apply_config_preset", {
      dir: "/my/project",
      preset: "Team Conventions",
    });
  });

  it("uses a manually specified configuration file", async () => {
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      load_lint_config_from_path: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() => expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true));
    document.querySelector<HTMLInputElement>("#config-picker-path-input")!.value = "/profiles/strict.yaml";
    document.querySelector<HTMLButtonElement>("#config-picker-use-path")!.click();
    await pending;

    expect(configPathOverride()).toBe("/profiles/strict.yaml");
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", { path: "/profiles/strict.yaml" });
  });

  it("does not re-show the picker for a directory already confirmed this session", async () => {
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await loadProjectConfigConfirmed("/my/project");
    invokeMock.mockClear();
    invokeImplFor({
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await loadProjectConfig("/my/project");

    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(false);
  });
});

describe("loadProjectInfo / applyProjectInfoToUI", () => {
  it("loads project information from the backend", async () => {
    const info = {
      detected_script_roots: ["/project/scripts/source"],
      used_configuration_file: "/project/papyrus-lint.yaml",
    };
    invokeImplFor({ load_project_info: () => info });

    await expect(loadProjectInfo("/project")).resolves.toEqual(info);
    expect(invokeMock).toHaveBeenCalledWith("load_project_info", { dir: "/project" });
  });

  it("shows explicit empty-state messages", () => {
    applyProjectInfoToUI({ detected_script_roots: [], used_configuration_file: null });

    expect(document.querySelector("#detected-script-roots")!.textContent).toBe("None detected");
    expect(document.querySelector("#used-configuration-file")!.textContent).toBe(
      "None (using defaults)",
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

describe("loadRuleTags / applyRuleTags", () => {
  const trailingWhitespaceTags: RuleTagsInfo = {
    rule: "trailing-whitespace",
    description: "Test description for trailing whitespace.",
    kinds: ["style"],
    importance: "low",
    auto_fixable: true,
    doc_url: "https://papyrus-lint.idrinth.de/#lint-trailing-whitespace",
  };

  afterEach(() => {
    // ruleTagsByRule is module state that outlives mountFixture(); reset it
    // so it doesn't leak into later tests that assume no tags are known.
    applyRuleTags([]);
  });

  it("returns the backend's reported rule tags", async () => {
    invokeImplFor({ list_rule_tags: () => [trailingWhitespaceTags] });

    await expect(loadRuleTags()).resolves.toEqual([trailingWhitespaceTags]);
    expect(invokeMock).toHaveBeenCalledWith("list_rule_tags");
  });

  it("returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("command not found"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadRuleTags()).resolves.toEqual([]);
  });

  it("applyRuleTags indexes tags by rule id for tagsForFinding", () => {
    applyRuleTags([trailingWhitespaceTags]);

    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toEqual(
      trailingWhitespaceTags,
    );
    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "unknown-rule" })).toBeUndefined();
    expect(tagsForFinding({ line: 1, column: 1, message: "x" })).toBeUndefined();
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

describe("loadCompileCheck / saveCompileCheck", () => {
  it("loadCompileCheck returns the backend's stored setting", async () => {
    invokeImplFor({ load_compile_check: () => true });

    await expect(loadCompileCheck("/proj")).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("load_compile_check", { dir: "/proj" });
  });

  it("loadCompileCheck returns false on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadCompileCheck("/proj")).resolves.toBe(false);
  });

  it("saveCompileCheck forwards the setting to the backend", async () => {
    invokeImplFor({ save_compile_check: () => undefined });

    await expect(saveCompileCheck("/proj", true)).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_compile_check", { dir: "/proj", enabled: true });
  });

  it("saveCompileCheck swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveCompileCheck("/proj", true)).resolves.toBeUndefined();
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

describe("loadLookupScriptRoots / saveLookupScriptRoots", () => {
  it("loadLookupScriptRoots returns the backend's configured roots", async () => {
    invokeImplFor({
      load_lookup_script_roots: () => ["C:/Skyrim/Data/Scripts/Source", "C:/Skyrim/Data/Source/Scripts"],
    });

    await expect(loadLookupScriptRoots("/proj")).resolves.toEqual([
      "C:/Skyrim/Data/Scripts/Source",
      "C:/Skyrim/Data/Source/Scripts",
    ]);
    expect(invokeMock).toHaveBeenCalledWith("load_lookup_script_roots", { dir: "/proj" });
  });

  it("loadLookupScriptRoots returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadLookupScriptRoots("/proj")).resolves.toEqual([]);
  });

  it("saveLookupScriptRoots forwards the roots to the backend", async () => {
    invokeImplFor({ save_lookup_script_roots: () => undefined });

    await expect(
      saveLookupScriptRoots("/proj", ["C:/Skyrim/Data/Scripts/Source"]),
    ).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_lookup_script_roots", {
      dir: "/proj",
      roots: ["C:/Skyrim/Data/Scripts/Source"],
    });
  });

  it("saveLookupScriptRoots swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      saveLookupScriptRoots("/proj", ["C:/Skyrim/Data/Scripts/Source"]),
    ).resolves.toBeUndefined();
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

  it("invokes onOutcome as each file finishes, without waiting for the rest of the batch", async () => {
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => ((args as { path: string }).path === "B.psc" ? pendingB : []),
    });

    const seen: string[] = [];
    const result = parsePscFiles(["A.psc", "B.psc"], (outcome) => seen.push(outcome.path));

    for (let i = 0; i < 5; i++) {
      await Promise.resolve();
    }
    expect(seen).toEqual(["A.psc"]);

    resolveB([]);
    await result;
    expect(seen).toEqual(["A.psc", "B.psc"]);
  });

  it("caps concurrent work at the machine's hardware concurrency instead of starting every file at once", async () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "hardwareConcurrency");
    Object.defineProperty(navigator, "hardwareConcurrency", { value: 2, configurable: true });
    try {
      const started: string[] = [];
      const pendingResolvers = new Map<string, (findings: Diagnostic[]) => void>();
      invokeImplFor({
        parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
        lint_psc_file: (args) => {
          const path = (args as { path: string }).path;
          started.push(path);
          return new Promise<Diagnostic[]>((resolve) => {
            pendingResolvers.set(path, resolve);
          });
        },
      });

      const result = parsePscFiles(["A.psc", "B.psc", "C.psc"]);

      for (let i = 0; i < 5; i++) {
        await Promise.resolve();
      }
      expect([...started].sort()).toEqual(["A.psc", "B.psc"]);

      pendingResolvers.get("A.psc")?.([]);
      for (let i = 0; i < 5; i++) {
        await Promise.resolve();
      }
      expect([...started].sort()).toEqual(["A.psc", "B.psc", "C.psc"]);

      pendingResolvers.get("B.psc")?.([]);
      pendingResolvers.get("C.psc")?.([]);
      const outcomes = await result;
      expect(outcomes.map((outcome) => outcome.path)).toEqual(["A.psc", "B.psc", "C.psc"]);
    } finally {
      if (original) {
        Object.defineProperty(navigator, "hardwareConcurrency", original);
      }
    }
  });

  it("lint_psc_file forwards the currently configured compiler path and compile-check setting", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe",
      load_compile_check: () => true,
      load_script_roots: () => [],
      load_lookup_script_roots: () => ["C:/Skyrim/Data/Scripts/Source"],
      parse_psc_file: () => ({ name: "MyScript" }),
      lint_psc_file: () => [],
    });
    await useProjectDir("/proj");

    await parsePscFiles(["/scripts/MyScript.psc"]);

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", {
      path: "/scripts/MyScript.psc",
      root: "/proj",
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: ["C:/Skyrim/Data/Scripts/Source"],
      compilerPath: "C:\\Tools\\PapyrusCompiler.exe",
      compileCheck: true,
    });
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
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
    });
  });

  it("previewRepairPscFile forwards to the preview_repair_psc_file command", async () => {
    const diff = "--- /scripts/MyScript.psc\n+++ /scripts/MyScript.psc\n@@ -1,1 +1,1 @@\n-old\n+new\n";
    invokeImplFor({ preview_repair_psc_file: () => diff });

    await expect(previewRepairPscFile("/scripts/MyScript.psc")).resolves.toEqual(diff);
    expect(invokeMock).toHaveBeenCalledWith("preview_repair_psc_file", {
      path: "/scripts/MyScript.psc",
      config: expect.anything(),
    });
  });

  it("repairPscFinding forwards the rule and line to the repair_psc_finding command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_finding: () => remaining });

    await expect(repairPscFinding("/scripts/MyScript.psc", "comma-spacing", 3)).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_finding", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rule: "comma-spacing",
      line: 3,
    });
  });

  it("repairPscFileRule forwards the rule, but no line, to the repair_psc_file_rule command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_file_rule: () => remaining });

    await expect(repairPscFileRule("/scripts/MyScript.psc", "trailing-whitespace")).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rule: "trailing-whitespace",
    });
  });

  it("addDisableCommentToPscLine forwards the rules and line to the add_disable_comment_to_psc_line command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ add_disable_comment_to_psc_line: () => remaining });

    await expect(
      addDisableCommentToPscLine("/scripts/MyScript.psc", ["comma-spacing", "trailing-whitespace"], 3),
    ).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("add_disable_comment_to_psc_line", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rules: ["comma-spacing", "trailing-whitespace"],
      line: 3,
    });
  });
});

describe("showLintProgress / updateLintProgress / hideLintProgress", () => {
  it("shows the progress bar reset to 0/total", () => {
    showLintProgress(3);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(0);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.max).toBe(3);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 0 / 3 files");
  });

  it("stays hidden when there are no files to process", () => {
    showLintProgress(0);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
  });

  it("updates the bar's value and label as files finish", () => {
    showLintProgress(2);
    updateLintProgress(1, 2);

    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(1);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 1 / 2 files");
  });

  it("hides the progress bar", () => {
    showLintProgress(2);
    hideLintProgress();

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
  });

  it("scheduleHideLintProgress keeps the bar visible during the grace period, then hides it", () => {
    vi.useFakeTimers();
    showLintProgress(2);
    updateLintProgress(2, 2);

    scheduleHideLintProgress();
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(1999);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(1);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
  });

  it("scheduleHideLintProgress's pending hide is cancelled by a new showLintProgress call", () => {
    vi.useFakeTimers();
    showLintProgress(2);
    updateLintProgress(2, 2);
    scheduleHideLintProgress();

    showLintProgress(3);
    vi.advanceTimersByTime(2000);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    vi.useRealTimers();
  });

  it("scheduleHideLintProgress replaces an already pending hide", () => {
    vi.useFakeTimers();
    showLintProgress(1);

    scheduleHideLintProgress(1000);
    vi.advanceTimersByTime(500);
    scheduleHideLintProgress(1000);
    vi.advanceTimersByTime(500);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(500);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
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
    showResult("/proj/a.achlist", ["/proj/one.psc", "/proj/two.psc"], "/proj");

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/a.achlist");
    expect(document.querySelectorAll("#achlist-result-list > li")).toHaveLength(2);
    expect(document.querySelector("#achlist-result")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelector<HTMLElement>("#panel-files")!.hidden).toBe(false);
  });

  it("showResult displays entries relative to the given base directory", () => {
    showResult("/proj/a.achlist", ["/proj/scripts/source/one.psc", "/proj/readme.txt"], "/proj");

    const items = document.querySelectorAll("#achlist-result-list > li span");
    expect(items[0].textContent).toBe("scripts/source/one.psc");
    expect(items[1].textContent).toBe("readme.txt");
  });

  it("showResult falls back to the absolute path when no base directory is known", () => {
    showResult("/proj/a.achlist", ["/proj/one.psc"], null);

    expect(document.querySelector("#achlist-result-list > li span")!.textContent).toBe("/proj/one.psc");
  });

  it("showResult adds a View button only for .psc entries", () => {
    showResult("/proj/a.achlist", ["/proj/one.psc", "/proj/readme.txt"], "/proj");

    const rows = document.querySelectorAll("#achlist-result-list > li");
    expect(rows[0].querySelector(".achlist-result__view-button")).not.toBeNull();
    expect(rows[1].querySelector(".achlist-result__view-button")).toBeNull();
  });

  it("showResult's View button opens the code viewer for that .psc file, marking its current findings", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["one.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "One" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] risky" }],
      read_psc_file: () => 'Debug.Trace("hi")',
    });

    // Populates currentPscOutcomes (via the lint pass) so the View button
    // rendered by showResult below has real findings to look up.
    const pending = handleDroppedPaths(["/proj/a.achlist"]);
    await confirmDetectedConfig();
    await pending;

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();
    await Promise.resolve();
    await Promise.resolve();

    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    expect(dialog.hasAttribute("open")).toBe(true);
    expect(document.querySelector("#code-viewer-title")!.textContent).toBe("one.psc");
    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      true,
    );
  });

  it("showResult's View button opens files that have no current lint outcome", async () => {
    invokeImplFor({ read_psc_file: () => "ScriptName Unlinted" });
    showResult("/proj/a.achlist", ["/proj/unlinted.psc"], "/proj");

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();

    await vi.waitFor(() => {
      expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(true);
    });
    expect(document.querySelector("#code-viewer-title")!.textContent).toMatch(/unlinted\.psc$/);
    expect(document.querySelectorAll(".code-viewer__line--error, .code-viewer__line--warning")).toHaveLength(0);
  });
});

describe("handleDroppedPaths", () => {
  it("rejects a drop with no .achlist file and more than one file", async () => {
    await handleDroppedPaths(["/scripts/A.psc", "/scripts/B.psc"]);
    expect(document.querySelector("#drop-zone-error")!.textContent).toContain(".achlist");
  });

  it("rejects a drop of a single file that's neither .achlist nor .psc", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => Promise.reject(new Error("not a directory")),
    });

    await handleDroppedPaths(["/scripts/readme.txt"]);

    expect(document.querySelector("#drop-zone-error")!.textContent).toContain(".psc");
  });

  it("recursively scans a dropped directory with no .achlist, loading project config and linting each .psc found", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => ["/proj/scripts/source/A.psc", "/proj/scripts/source/Requiem/B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/scripts/source"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/scripts/source");
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/A.psc" });
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/Requiem/B.psc" });
  });

  it("falls back to the dropped directory itself as project root when no scripts/source pair is found", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => ["/proj/Nested/A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj"]);
    await confirmDetectedConfig();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/proj" });
  });

  it("parses the achlist, loads project config, and lints each .psc entry", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "readme.txt"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/list.achlist");
    // readme.txt isn't a .psc file, so only A.psc should have been linted.
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "A.psc" });
    expect(invokeMock).not.toHaveBeenCalledWith("parse_psc_file", { path: "readme.txt" });
  });

  it("resolves the project root from a resolved script's own position when the achlist itself lives elsewhere", async () => {
    // Users sometimes drop the .achlist somewhere other than the project
    // root (e.g. next to a game's Data directory) while the actual
    // project, including its papyrus-lint.yaml, lives deeper:
    //
    //   /proj/list.achlist
    //   /proj/somefolder/otherfolder/scripts/source/AType.psc
    //   /proj/somefolder/otherfolder/source/scripts/BType.psc
    //
    // The achlist's own parent directory ("/proj") isn't the real project
    // root, so it must instead be found from the resolved scripts' own
    // position under their scripts/source or source/scripts pair.
    invokeImplFor({
      parse_achlist_file: () => [
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
        "/proj/somefolder/otherfolder/source/scripts/BType.psc",
      ],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "AType" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/proj/somefolder/otherfolder" });
  });

  it("shows an error when the achlist itself fails to parse", async () => {
    invokeMock.mockRejectedValue(new Error("bad json"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleDroppedPaths(["/proj/list.achlist"]);

    expect(document.querySelector("#drop-zone-error")!.textContent).toContain("Failed to read");
  });

  it("lints a single dropped .psc file, resolving the project root two directories up", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/scripts/source/A.psc"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe(
      "Loaded /proj/scripts/source/A.psc",
    );
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/A.psc" });
  });

  it("clears previous findings before re-rendering, so re-dropping the same achlist can't show stale diagnostics while it reloads", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] stale finding" }],
    });
    const firstDrop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await firstDrop;

    let resolveLint: (findings: Diagnostic[]) => void = () => {};
    const pendingLint = new Promise<Diagnostic[]>((resolve) => {
      resolveLint = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => pendingLint,
      read_psc_file: () => 'Debug.Trace("hi")',
    });

    const secondDrop = handleDroppedPaths(["/proj/list.achlist"]);
    // Let the re-render happen, but not the re-lint pass, which is still
    // blocked on pendingLint.
    for (let i = 0; i < 5; i++) {
      await Promise.resolve();
    }

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);

    resolveLint([]);
    await secondDrop;
  });

  it("switches to the lint tab and grows the results list as each file finishes, without waiting for the whole achlist", async () => {
    vi.useFakeTimers();
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path === "B.psc"
          ? pendingB
          : [{ line: 1, column: 1, message: "[warning] from A" }],
    });

    switchTab("import");
    const drop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    // Let A's parse+lint pass resolve, but not B's, which is still pending.
    for (let i = 0; i < 30; i++) {
      await Promise.resolve();
    }

    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(1);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.max).toBe(2);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 1 / 2 files");

    resolveB([{ line: 2, column: 1, message: "[warning] from B" }]);
    await drop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(2);
    // The finished progress bar stays visible for a grace period instead of
    // disappearing the instant the last file finishes.
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(2000);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
  });

  it("does not force the user back to the lint tab if they switch away while later files are still being analyzed", async () => {
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path === "B.psc"
          ? pendingB
          : [{ line: 1, column: 1, message: "[warning] from A" }],
    });

    const drop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    // Let A's parse+lint pass resolve, but not B's, which is still pending.
    for (let i = 0; i < 30; i++) {
      await Promise.resolve();
    }
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);

    // The user switches away from the Lint results tab while B is still
    // being analyzed in the background.
    switchTab("settings");

    resolveB([{ line: 2, column: 1, message: "[warning] from B" }]);
    await drop;

    // B finishing (and re-rendering the results list) must not have yanked
    // the user back to the Lint results tab.
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-settings")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(2);
  });

  it("ignores a stale drop's straggling outcome once a newer drop has started, instead of mixing it into the newer results", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    let resolveOldLintStarted: () => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    const oldLintStarted = new Promise<void>((resolve) => {
      resolveOldLintStarted = resolve;
    });
    invokeImplFor({
      parse_achlist_file: (args) =>
        (args as { path: string }).path === "/proj/old.achlist" ? ["Old.psc"] : ["New.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => {
        if ((args as { path: string }).path === "Old.psc") {
          resolveOldLintStarted();
          return pendingOldLint;
        }
        return [{ line: 1, column: 1, message: "[warning] from New" }];
      },
    });

    const oldDrop = handleDroppedPaths(["/proj/old.achlist"]);
    await confirmDetectedConfig();
    await oldLintStarted;

    // A second, newer drop starts while the first one is still stuck
    // linting Old.psc. Both achlists resolve to the same "/proj" project
    // directory, already confirmed by oldDrop above, so this one doesn't
    // show the picker again.
    const newDrop = handleDroppedPaths(["/proj/new.achlist"]);
    await newDrop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector("#psc-result-list")!.textContent).toContain("New.psc");

    // The stale drop's lint pass finally finishes; its outcome must not be
    // mixed into the newer drop's already-rendered results.
    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
  });

  it("ignores a stale single-psc drop's outcome after a newer psc drop has finished", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path.endsWith("Old.psc")
          ? pendingOldLint
          : [{ line: 1, column: 1, message: "[warning] from New" }],
    });

    const oldDrop = handleDroppedPaths(["/proj/scripts/source/Old.psc"]);
    await confirmDetectedConfig();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    // Both .psc paths resolve to the same "/proj" project directory,
    // already confirmed above, so this one doesn't show the picker again.
    const newDrop = handleDroppedPaths(["/proj/scripts/source/New.psc"]);
    await newDrop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector("#psc-result-list")!.textContent).toContain("New.psc");

    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
    expect(items[0].textContent).not.toContain("Old.psc");
  });

  it("ignores a stale directory scan's outcome after a newer directory scan has finished", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    let resolveOldLintStarted: () => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    const oldLintStarted = new Promise<void>((resolve) => {
      resolveOldLintStarted = resolve;
    });
    invokeImplFor({
      list_psc_files_recursively: (args) =>
        (args as { path: string }).path === "/proj/old"
          ? ["/proj/old/Old.psc"]
          : ["/proj/new/New.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => {
        if ((args as { path: string }).path.endsWith("Old.psc")) {
          resolveOldLintStarted();
          return pendingOldLint;
        }
        return [{ line: 1, column: 1, message: "[warning] from New" }];
      },
    });

    const oldDrop = handleDroppedPaths(["/proj/old"]);
    await confirmDetectedConfig();
    await oldLintStarted;

    // "/proj/new" is a different, not-yet-confirmed project directory, so
    // this drop shows its own picker too.
    const newDrop = handleDroppedPaths(["/proj/new"]);
    await confirmDetectedConfig();
    await newDrop;

    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
    expect(items[0].textContent).not.toContain("Old.psc");
  });
});

describe("relintCurrentFiles / Lint results tab settings staleness", () => {
  async function dropOneFileAndSettle() {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => undefined,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] from first pass" }],
    });
    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  }

  it("relintCurrentFiles does nothing when no files are currently loaded", async () => {
    // An achlist with no .psc entries leaves currentPscOutcomes empty,
    // giving a deterministic "nothing loaded" state to start the test from
    // regardless of what an earlier test left behind (module-level state
    // isn't reset between tests).
    invokeImplFor({
      parse_achlist_file: () => ["readme.txt"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;
    invokeMock.mockClear();

    await relintCurrentFiles();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("clicking the Lint results tab after a settings change clears the list and re-lints the same files", async () => {
    await dropOneFileAndSettle();

    switchTab("settings");
    invokeImplFor({
      save_lint_config: () => undefined,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[error] from second pass" }],
    });
    handleLintConfigChanged();
    invokeMock.mockClear();

    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    // The results panel is hidden synchronously (the same "empty" signal a
    // fresh drop uses while its own lint pass is still running) before the
    // re-lint pass's async work (parse_psc_file/lint_psc_file) runs.
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);

    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "A.psc" });
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(false);
    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("from second pass");
  });

  it("clicking the Lint results tab with no settings change since the last lint just switches tabs", async () => {
    await dropOneFileAndSettle();

    switchTab("settings");
    invokeMock.mockClear();

    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    await Promise.resolve();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  });

  it("re-marks results stale once a config-path-override reload settles, even if a race relint already ran against the old config", async () => {
    // Regression test: handleConfigPathOverrideChanged sets lintResultsStale
    // before its useProjectDir() reload (which replaces currentLintConfig)
    // has actually finished. If the Lint results tab is clicked in that
    // window, relintCurrentFiles relints against the still-old config and
    // clears the flag - so once the reload finally lands the new config, the
    // displayed results must be re-flagged stale, or nothing ever re-lints
    // them against it.
    await dropOneFileAndSettle();

    let resolveOverrideLoad: (config: LintConfig) => void = () => {};
    const pendingOverrideLoad = new Promise<LintConfig>((resolve) => {
      resolveOverrideLoad = resolve;
    });
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({
      load_lint_config_from_path: () => pendingOverrideLoad,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] raced" }],
    });
    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "/profiles/strict.yaml";
    const overrideChange = handleConfigPathOverrideChanged();

    // The override's own config load is still pending, but lintResultsStale
    // was already set before it started; switching to the Lint tab now
    // races ahead of it and relints against the config still in effect.
    invokeMock.mockClear();
    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }
    const raceLintCall = invokeMock.mock.calls.find(([command]) => command === "lint_psc_file");
    expect((raceLintCall?.[1] as { config: LintConfig }).config.semicolon).toBe(false);

    // The override's config load now finishes, well after that race relint
    // already cleared lintResultsStale.
    invokeMock.mockClear();
    resolveOverrideLoad(custom);
    await overrideChange;

    invokeImplFor({
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] under custom config" }],
    });
    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    const secondLintCall = invokeMock.mock.calls.find(([command]) => command === "lint_psc_file");
    expect((secondLintCall?.[1] as { config: LintConfig }).config.semicolon).toBe(true);
  });
});

describe("projectDirForPscPath", () => {
  it("resolves the project root two directories above the script's own directory", () => {
    expect(projectDirForPscPath("/proj/scripts/source/A.psc")).toBe("/proj");
    expect(projectDirForPscPath("C:\\proj\\scripts\\source\\A.psc")).toBe("C:\\proj");
  });
});

describe("findCandidatePairRoot", () => {
  it("finds the root above a scripts/source pair", () => {
    expect(findCandidatePairRoot("/proj/scripts/source/A.psc")).toBe("/proj");
  });

  it("finds the root above a source/scripts pair", () => {
    expect(findCandidatePairRoot("/proj/source/scripts/A.psc")).toBe("/proj");
  });

  it("matches case-insensitively and across backslash paths", () => {
    expect(findCandidatePairRoot("C:\\proj\\Scripts\\Source\\A.psc")).toBe("C:\\proj");
  });

  it("finds the root for a script nested under a namespaced subfolder", () => {
    expect(findCandidatePairRoot("/proj/scripts/source/User/A.psc")).toBe("/proj");
  });

  it("returns null when no scripts/source or source/scripts pair is present", () => {
    expect(findCandidatePairRoot("/proj/other/A.psc")).toBeNull();
  });
});

describe("projectDirForAchlist", () => {
  it("resolves the conventional layout where the achlist already lives in the project root", () => {
    expect(projectDirForAchlist("/proj/list.achlist", ["/proj/scripts/source/A.psc"])).toBe("/proj");
  });

  it("falls back to a resolved entry's own scripts/source position when the achlist lives elsewhere", () => {
    expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
        "/proj/somefolder/otherfolder/source/scripts/BType.psc",
      ]),
    ).toBe("/proj/somefolder/otherfolder");
  });

  it("falls back to the achlist's parent directory when no entry matches the convention", () => {
    expect(projectDirForAchlist("/proj/list.achlist", ["/proj/other/A.psc"])).toBe("/proj");
  });

  it("ignores non-.psc entries when looking for a matching script position", () => {
    expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/readme.txt",
        "/proj/somefolder/scripts/source/A.psc",
      ]),
    ).toBe("/proj/somefolder");
  });
});

describe("projectDirForDirectory", () => {
  it("resolves a nested scripts/source pair beneath the dropped directory", () => {
    expect(
      projectDirForDirectory("/proj/scripts/source", ["/proj/scripts/source/Requiem/A.psc"]),
    ).toBe("/proj");
  });

  it("falls back to the dropped directory itself when no entry matches the convention", () => {
    expect(projectDirForDirectory("/proj", ["/proj/Nested/A.psc"])).toBe("/proj");
  });

  it("falls back to the dropped directory itself for an empty scan", () => {
    expect(projectDirForDirectory("/proj", [])).toBe("/proj");
  });
});

describe("scriptRootsForAchlist", () => {
  it("uses every distinct directory containing a listed Papyrus source", () => {
    expect(
      scriptRootsForAchlist([
        "/proj/source/dir/one/Script1.psc",
        "/proj/source/dir/two/Script2.PSC",
        "/proj/source/dir/one/Script3.psc",
        "/proj/readme.txt",
      ]),
    ).toEqual(["/proj/source/dir/one", "/proj/source/dir/two"]);
  });
});

describe("wired DOM interactions", () => {
  it("loads startup metadata and applies theme changes through the registered listeners", async () => {
    const version = document.createElement("span");
    version.id = "app-version";
    document.body.append(version);
    invokeImplFor({
      get_app_version: () => "1.2.3",
      list_rule_tags: () => [],
      list_config_presets: () => [],
    });

    // Re-dispatch startup after installing deterministic backend handlers;
    // mountFixture's initial dispatch intentionally happens before each test
    // has configured the command mock.
    document.dispatchEvent(new Event("DOMContentLoaded", { bubbles: true }));
    await vi.waitFor(() => expect(version.textContent).toBe("v1.2.3"));

    const theme = document.querySelector<HTMLSelectElement>("#theme-select")!;
    theme.value = "dark";
    theme.dispatchEvent(new Event("change"));
    expect(localStorage.getItem("papyrus-lint:theme")).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("closes the configuration picker only when its backdrop is clicked", () => {
    const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
    picker.showModal();

    picker.querySelector(".config-picker__title")!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(picker.open).toBe(true);

    picker.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(picker.open).toBe(false);
  });

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
      "#bool-like-int",
      "#assume-auto-properties-filled",
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
      load_compile_check: () => false,
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
      findings: [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }],
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
    const gutter = document.querySelector<HTMLElement>("#code-viewer-editor-gutter")!;

    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();
    textarea.value = "Int x = 2\n";
    textarea.dispatchEvent(new Event("input"));
    textarea.scrollTop = 12;
    textarea.scrollLeft = 7;
    textarea.dispatchEvent(new Event("scroll"));
    expect(highlight.scrollTop).toBe(12);
    expect(highlight.scrollLeft).toBe(7);
    expect(gutter.scrollTop).toBe(12);

    textarea.dispatchEvent(new MouseEvent("mousemove", { clientY: 20 }));
    textarea.dispatchEvent(new Event("scroll"));
    textarea.dispatchEvent(new MouseEvent("mouseleave"));
    expect(textarea.title).toBe("");

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

  it("reports repair failures without rejecting", async () => {
    invokeMock.mockRejectedValue(new Error("repair failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const button = document.createElement("button");
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "parsed", findings: [] };
    await expect(handleFixClick("/a.psc", outcome, button)).resolves.toBeUndefined();
    expect(button.disabled).toBe(true);
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

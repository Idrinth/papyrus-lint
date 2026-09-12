import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
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
  applyConfigPreset,
  applyProjectInfoToUI,
  applyRuleTags,
  applyTheme,
  applyScriptRootsToUI,
  buildPscResultItem,
  cancelCodeViewerEditMode,
  clearError,
  collectFilteredIssues,
  configPathOverride,
  deleteUserPreset,
  dirnameOf,
  enterCodeViewerEditMode,
  escapeAttr,
  exportUserPreset,
  findCandidatePairRoot,
  formatIssuesAsJson,
  formatIssuesAsText,
  formatIssuesForAi,
  getPresetLintConfig,
  handleAutocompleteKeydown,
  handleCodeViewerFixClick,
  handleCodeViewerPreviewFixClick,
  handleCompileClick,
  handleCompileCheckChanged,
  handleCompilerPathChanged,
  handleConfigPathOverrideChanged,
  handleDeletePresetClick,
  handleDroppedPaths,
  handleEditorTabKeydown,
  handleExportAiClick,
  handleExportIssuesClick,
  handleExportPresetClick,
  handleFixClick,
  handleFixIssueClick,
  handleLintConfigChanged,
  handleMassFixClick,
  handleRenamePresetClick,
  handleResetToPresetClick,
  handleSaveConfigAsPresetClick,
  handleScriptRootsChanged,
  hasFixableFindings,
  hideAutocomplete,
  hideLintProgress,
  isAchlistPath,
  isCodeViewerEditDirty,
  isCustomPreset,
  isFixableFinding,
  isPscPath,
  levelOf,
  lintConfigFromUI,
  listScriptMembers,
  loadAppVersion,
  loadCompileCheck,
  loadCompilerPath,
  loadConfigPresets,
  loadLintConfig,
  loadLintConfigFromPath,
  loadProjectConfig,
  loadProjectInfo,
  loadRuleTags,
  loadStoredTheme,
  loadScriptRoots,
  massFixRuleCounts,
  massFixRuleDisplayName,
  matchesFilenameFilter,
  matchesTagFilters,
  openCodeViewer,
  parsePscFiles,
  populateResetPresetSelect,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPscPath,
  promptForConfigSelection,
  refreshPresetManagementTab,
  relativePath,
  relintCurrentFiles,
  renameUserPreset,
  renderMassFixList,
  renderPresetManagementTab,
  renderPscResults,
  previewRepairPscFile,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  requestCloseCodeViewer,
  resetConfirmedProjectDirs,
  saveAndCompileCodeViewerEdits,
  saveCodeViewerEdits,
  saveCompileCheck,
  saveCompilerPath,
  saveLintConfig,
  saveLintConfigToPath,
  saveScriptRoots,
  scheduleHideLintProgress,
  scriptRootsFromUI,
  scriptRootsForAchlist,
  setSettingsLocked,
  severityOf,
  showError,
  showLintProgress,
  showResult,
  storeTheme,
  switchTab,
  tagsForFinding,
  toggleCodeViewerFullscreen,
  updateAutocomplete,
  updateExportIssuesButtonState,
  updateLintProgress,
  useProjectDir,
  type ConfigSelectionResult,
  type Diagnostic,
  type LintConfig,
  type PscParseOutcome,
  type RuleTagsInfo,
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
  resetConfirmedProjectDirs();
});

// Waits for the "select this project's configuration" dialog
// (promptForConfigSelection) to open and clicks "Continue", accepting
// useProjectDir's own auto-detection either way (an existing configuration
// file, or the engine's silent defaults if the project has none). Pumps
// microtasks directly instead of vi.waitFor, so it works the same whether
// or not a test has switched to fake timers.
async function confirmDetectedConfig(): Promise<void> {
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  expect(picker.hasAttribute("open")).toBe(true);
  document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
}

// Drives useProjectDir through loadProjectConfig's own "select this
// project's configuration" step for tests that don't care about that step
// itself, immediately accepting whatever useProjectDir would already do on
// its own (see confirmDetectedConfig). A directory already confirmed this
// session (see resetConfirmedProjectDirs) skips the dialog entirely, the
// same as loadProjectConfig itself does.
async function loadProjectConfigConfirmed(dir: string): Promise<void> {
  const pending = loadProjectConfig(dir);
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  if (picker.hasAttribute("open")) {
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
  }
  await pending;
}

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

describe("promptForConfigSelection", () => {
  it("shows the detected configuration and no preset list when one was found", async () => {
    const pending = promptForConfigSelection({
      detected_script_roots: [],
      used_configuration_file: "/proj/papyrus-lint.yaml",
    });

    expect(document.querySelector<HTMLElement>("#config-picker-detected")!.hidden).toBe(false);
    expect(document.querySelector("#config-picker-detected-path")!.textContent).toBe(
      "/proj/papyrus-lint.yaml",
    );
    expect(document.querySelector<HTMLElement>("#config-picker-none")!.hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("#config-picker-preset-list")!.hidden).toBe(true);

    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
    const result: ConfigSelectionResult = await pending;
    expect(result).toEqual({ kind: "detected" });
  });

  it("does not fetch presets when the project already has a configuration", async () => {
    invokeImplFor({
      list_config_presets: () => {
        throw new Error("presets should not be requested");
      },
    });
    // Ignore the startup refresh kicked off by main.ts's DOMContentLoaded
    // handler; this assertion is specifically about the picker invocation.
    await Promise.resolve();
    invokeMock.mockClear();

    const pending = promptForConfigSelection({
      detected_script_roots: [],
      used_configuration_file: "/proj/papyrus-lint.yaml",
    });
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();

    await expect(pending).resolves.toEqual({ kind: "detected" });
    expect(invokeMock).not.toHaveBeenCalledWith("list_config_presets");
  });

  it("shows the no-configuration notice and lists every preset when none was found", async () => {
    invokeImplFor({
      list_config_presets: () => [
        { id: "strict", label: "Strict", description: "Catches everything." },
        { id: "careful", label: "Careful", description: "The quietest option." },
      ],
    });

    void promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );

    expect(document.querySelector<HTMLElement>("#config-picker-detected")!.hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("#config-picker-none")!.hidden).toBe(false);
    const options = document.querySelectorAll<HTMLButtonElement>(
      "#config-picker-preset-list .config-picker__preset-option",
    );
    expect(options).toHaveLength(2);
    expect(options[1].textContent).toContain("Careful");
    expect(options[1].textContent).toContain("The quietest option.");
  });

  it("hides the preset list entirely when there are no presets to offer", async () => {
    invokeImplFor({ list_config_presets: () => [] });

    void promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );

    expect(document.querySelector<HTMLElement>("#config-picker-preset-list")!.hidden).toBe(true);
  });

  it("clears preset options left by an earlier selection", async () => {
    invokeImplFor({
      list_config_presets: () => [
        { id: "team-style", label: "Team Style", description: "A custom preset." },
      ],
    });
    const first = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelectorAll("#config-picker-preset-list .config-picker__preset-option")).toHaveLength(1),
    );
    document.querySelector<HTMLDialogElement>("#config-picker")!.close();
    await first;

    const second = promptForConfigSelection({
      detected_script_roots: [],
      used_configuration_file: "/next/papyrus-lint.yaml",
    });

    expect(document.querySelectorAll("#config-picker-preset-list .config-picker__preset-option")).toHaveLength(0);
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
    await second;
  });

  it("resolves detected when closed without a choice (Escape or a backdrop click)", async () => {
    const pending = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );

    document.querySelector<HTMLDialogElement>("#config-picker")!.close();

    await expect(pending).resolves.toEqual({ kind: "detected" });
  });

  it("resolves with the trimmed path once a different file is confirmed", async () => {
    const pending = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );

    document.querySelector<HTMLInputElement>("#config-picker-path-input")!.value = "  /profiles/strict.yaml  ";
    document.querySelector<HTMLButtonElement>("#config-picker-use-path")!.click();

    await expect(pending).resolves.toEqual({ kind: "path", path: "/profiles/strict.yaml" });
  });

  it("does not resolve when the different-file input is left blank", async () => {
    void promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );

    document.querySelector<HTMLButtonElement>("#config-picker-use-path")!.click();

    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true);
  });

  it("resolves with the chosen preset when one of the inline options is clicked", async () => {
    invokeImplFor({
      list_config_presets: () => [
        { id: "strict", label: "Strict", description: "Catches everything." },
        { id: "careful", label: "Careful", description: "The quietest option." },
      ],
    });

    const pending = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(
        document.querySelectorAll("#config-picker-preset-list .config-picker__preset-option").length,
      ).toBe(2),
    );
    document
      .querySelectorAll<HTMLButtonElement>("#config-picker-preset-list .config-picker__preset-option")[1]
      .click();

    await expect(pending).resolves.toEqual({ kind: "preset", preset: "careful" });
    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(false);
  });

  it("resolves with a user preset's id unchanged, not its label", async () => {
    invokeImplFor({
      list_config_presets: () => [
        { id: "Team Conventions", label: "Team Conventions", description: "A custom preset." },
      ],
    });

    const pending = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(
        document.querySelectorAll("#config-picker-preset-list .config-picker__preset-option").length,
      ).toBe(1),
    );
    const option = document.querySelector<HTMLButtonElement>(
      "#config-picker-preset-list .config-picker__preset-option",
    )!;
    expect(option.textContent).toContain("Team Conventions");
    expect(option.textContent).toContain("A custom preset.");
    option.click();

    await expect(pending).resolves.toEqual({ kind: "preset", preset: "Team Conventions" });
  });

  it("doesn't accumulate stale listeners on the static Continue/browse buttons across repeated calls", async () => {
    // Continue/the browse button are reused across every call (unlike the
    // preset options, rebuilt fresh each time); a leaked listener from an
    // earlier call resolving a different way would double-fire finish() on
    // a later call's own click.
    const first = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );
    document.querySelector<HTMLDialogElement>("#config-picker")!.close();
    await first;

    const second = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );
    document.querySelector<HTMLInputElement>("#config-picker-path-input")!.value = "/profiles/strict.yaml";
    document.querySelector<HTMLButtonElement>("#config-picker-use-path")!.click();

    await expect(second).resolves.toEqual({ kind: "path", path: "/profiles/strict.yaml" });
  });

  it("resolves immediately with detected when the dialog isn't present in the DOM", async () => {
    document.querySelector("#config-picker")!.remove();
    document.dispatchEvent(new Event("DOMContentLoaded", { bubbles: true }));

    await expect(
      promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null }),
    ).resolves.toEqual({ kind: "detected" });
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

describe("loadConfigPresets / applyConfigPreset", () => {
  it("fetches the list of built-in presets from the backend", async () => {
    const presets = [
      { id: "strict", label: "Strict", description: "Catches everything." },
      { id: "standard", label: "Standard", description: "A middle ground." },
    ];
    invokeImplFor({ list_config_presets: () => presets });

    await expect(loadConfigPresets()).resolves.toEqual(presets);
  });

  it("returns an empty array when fetching presets fails", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    invokeImplFor({});

    await expect(loadConfigPresets()).resolves.toEqual([]);
  });

  it("normalizes a null backend response to an empty preset list", async () => {
    invokeImplFor({ list_config_presets: () => null });

    await expect(loadConfigPresets()).resolves.toEqual([]);
  });

  it("applies the chosen preset to the given project directory", async () => {
    invokeImplFor({ apply_config_preset: () => undefined });

    await applyConfigPreset("/my/project", "careful");

    expect(invokeMock).toHaveBeenCalledWith("apply_config_preset", { dir: "/my/project", preset: "careful" });
  });

  it("logs and swallows an error applying a preset", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    invokeImplFor({});

    await expect(applyConfigPreset("/my/project", "careful")).resolves.toBeUndefined();
  });
});

describe("handleSaveConfigAsPresetClick", () => {
  beforeEach(async () => {
    invokeImplFor({
      load_lint_config: () => ({ ...DEFAULT_LINT_CONFIG, semicolon: true }),
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: "/proj/papyrus-lint.yaml",
      }),
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();
  });

  it("does nothing when the prompt is left blank", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("   ");

    await handleSaveConfigAsPresetClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing when the prompt is cancelled", async () => {
    vi.spyOn(window, "prompt").mockReturnValue(null);

    await handleSaveConfigAsPresetClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("saves a new preset without asking to overwrite when the name is unused", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("my-preset");
    const confirmSpy = vi.spyOn(window, "confirm");
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      list_config_presets: () => [{ id: "strict", label: "Strict", description: "Catches everything." }],
      save_config_as_preset: () => undefined,
    });

    await handleSaveConfigAsPresetClick();

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("save_config_as_preset", {
      config: expect.objectContaining({ semicolon: true }),
      name: "my-preset",
      overwrite: false,
    });
    expect(window.alert).toHaveBeenCalledWith('Saved preset "my-preset".');
  });

  it("trims the entered name and saves the latest settings edited in the UI", async () => {
    invokeImplFor({ save_lint_config: () => undefined });
    document.querySelector<HTMLSelectElement>("#semicolon-style")!.value = "forbid";
    await handleLintConfigChanged();
    invokeMock.mockClear();
    vi.spyOn(window, "prompt").mockReturnValue("  team-style  ");
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      list_config_presets: () => [],
      save_config_as_preset: () => undefined,
    });

    await handleSaveConfigAsPresetClick();

    expect(invokeMock).toHaveBeenCalledWith("save_config_as_preset", {
      config: expect.objectContaining({ semicolon: false }),
      name: "team-style",
      overwrite: false,
    });
  });

  it("asks to overwrite when a preset already exists under that name, matched case-insensitively", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("Strict-Custom");
    vi.spyOn(window, "confirm").mockReturnValue(false);
    invokeImplFor({
      list_config_presets: () => [{ id: "strict-custom", label: "Strict Custom", description: "" }],
    });

    await handleSaveConfigAsPresetClick();

    expect(window.confirm).toHaveBeenCalledWith('A preset named "Strict-Custom" already exists. Overwrite it?');
    expect(invokeMock).not.toHaveBeenCalledWith("save_config_as_preset", expect.anything());
  });

  it("overwrites the existing preset once confirmed", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("Strict-Custom");
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      list_config_presets: () => [{ id: "strict-custom", label: "Strict Custom", description: "" }],
      save_config_as_preset: () => undefined,
    });

    await handleSaveConfigAsPresetClick();

    expect(invokeMock).toHaveBeenCalledWith("save_config_as_preset", {
      config: expect.objectContaining({ semicolon: true }),
      name: "Strict-Custom",
      overwrite: true,
    });
  });

  it("reports and logs a failure from the backend", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("my-preset");
    vi.spyOn(window, "alert").mockImplementation(() => {});
    vi.spyOn(console, "error").mockImplementation(() => {});
    invokeImplFor({
      list_config_presets: () => [],
      save_config_as_preset: () => {
        throw new Error("disk full");
      },
    });

    await handleSaveConfigAsPresetClick();

    expect(window.alert).toHaveBeenCalledWith('Failed to save preset "my-preset": Error: disk full');
    expect(console.error).toHaveBeenCalled();
  });
});

describe("getPresetLintConfig", () => {
  it("fetches the named preset's lint settings from the backend", async () => {
    const config = { ...DEFAULT_LINT_CONFIG, semicolon: false };
    invokeImplFor({ get_preset_lint_config: () => config });

    await expect(getPresetLintConfig("careful")).resolves.toEqual(config);
    expect(invokeMock).toHaveBeenCalledWith("get_preset_lint_config", { preset: "careful" });
  });
});

describe("populateResetPresetSelect", () => {
  it("rebuilds the dropdown's options from the given presets", () => {
    populateResetPresetSelect([
      { id: "strict", label: "Strict", description: "" },
      { id: "team-style", label: "Team Style", description: "" },
    ]);

    const select = document.querySelector<HTMLSelectElement>("#reset-to-preset-select")!;
    expect(Array.from(select.options).map((option) => [option.value, option.textContent])).toEqual([
      ["strict", "Strict"],
      ["team-style", "Team Style"],
    ]);
  });

  it("keeps the previously selected preset selected across a rebuild", () => {
    populateResetPresetSelect([
      { id: "strict", label: "Strict", description: "" },
      { id: "careful", label: "Careful", description: "" },
    ]);
    const select = document.querySelector<HTMLSelectElement>("#reset-to-preset-select")!;
    select.value = "careful";

    populateResetPresetSelect([
      { id: "strict", label: "Strict", description: "" },
      { id: "careful", label: "Careful", description: "" },
      { id: "team-style", label: "Team Style", description: "" },
    ]);

    expect(select.value).toBe("careful");
  });
});

describe("handleResetToPresetClick", () => {
  beforeEach(async () => {
    invokeImplFor({
      load_lint_config: () => ({ ...DEFAULT_LINT_CONFIG, semicolon: true }),
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: "/proj/papyrus-lint.yaml",
      }),
    });
    await useProjectDir("/proj");
    populateResetPresetSelect([{ id: "careful", label: "Careful", description: "" }]);
    invokeMock.mockClear();
  });

  it("does nothing when no preset is selected", async () => {
    populateResetPresetSelect([]);

    await handleResetToPresetClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing when the confirmation is declined", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);

    await handleResetToPresetClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("confirms, then applies and persists the selected preset's settings", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const config = { ...DEFAULT_LINT_CONFIG, semicolon: false };
    invokeImplFor({
      get_preset_lint_config: () => config,
      save_lint_config: () => undefined,
    });

    await handleResetToPresetClick();
    await Promise.resolve();

    expect(window.confirm).toHaveBeenCalledWith(
      'Reset all lint rule and formatting settings to the "Careful" preset? ' +
        "This overwrites your current settings and can't be undone.",
    );
    expect(invokeMock).toHaveBeenCalledWith("get_preset_lint_config", { preset: "careful" });
    expect(document.querySelector<HTMLSelectElement>("#semicolon-style")!.value).toBe("forbid");
    expect(invokeMock).toHaveBeenCalledWith("save_lint_config", {
      dir: "/proj",
      config: expect.objectContaining({ semicolon: false }),
    });
  });

  it("reports and logs a failure from the backend", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.spyOn(window, "alert").mockImplementation(() => {});
    vi.spyOn(console, "error").mockImplementation(() => {});
    invokeImplFor({
      get_preset_lint_config: () => {
        throw new Error("unknown preset");
      },
    });

    await handleResetToPresetClick();

    expect(window.alert).toHaveBeenCalledWith('Failed to reset settings to "Careful": Error: unknown preset');
    expect(console.error).toHaveBeenCalled();
  });
});

describe("isCustomPreset", () => {
  it("treats the three built-in preset ids as non-custom, case-insensitively", () => {
    expect(isCustomPreset({ id: "strict", label: "Strict", description: "" })).toBe(false);
    expect(isCustomPreset({ id: "Standard", label: "Standard", description: "" })).toBe(false);
    expect(isCustomPreset({ id: "CAREFUL", label: "Careful", description: "" })).toBe(false);
  });

  it("treats any other id as a custom preset", () => {
    expect(isCustomPreset({ id: "team-style", label: "Team Style", description: "" })).toBe(true);
  });
});

describe("renderPresetManagementTab", () => {
  const builtIns = [
    { id: "strict", label: "Strict", description: "Catches everything." },
    { id: "standard", label: "Standard", description: "A middle ground." },
    { id: "careful", label: "Careful", description: "The quietest option." },
  ];

  it("hides the tab and clears the list when there are no custom presets", () => {
    renderPresetManagementTab(builtIns);

    expect(document.querySelector("#tab-presets")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector("#preset-management-list")!.children.length).toBe(0);
  });

  it("shows the tab and lists only the custom presets", () => {
    renderPresetManagementTab([
      ...builtIns,
      { id: "team-style", label: "Team Style", description: "Our house rules." },
    ]);

    expect(document.querySelector("#tab-presets")!.hasAttribute("hidden")).toBe(false);
    const items = document.querySelectorAll("#preset-management-list .preset-management__item");
    expect(items.length).toBe(1);
    expect(items[0].querySelector(".preset-management__label")!.textContent).toBe("Team Style");
    expect(items[0].querySelectorAll(".preset-management__button").length).toBe(3);
  });

  it("switches back to the Settings tab if the active Presets tab's last custom preset disappears", () => {
    renderPresetManagementTab([...builtIns, { id: "team-style", label: "Team Style", description: "" }]);
    switchTab("presets");
    expect(document.querySelector("#tab-presets")!.classList.contains("tabs__tab--active")).toBe(true);

    renderPresetManagementTab(builtIns);

    expect(document.querySelector("#tab-presets")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector("#tab-settings")!.classList.contains("tabs__tab--active")).toBe(true);
    expect(document.querySelector("#panel-presets")!.hasAttribute("hidden")).toBe(true);
  });
});

describe("refreshPresetManagementTab / rename/delete/exportUserPreset", () => {
  it("refreshPresetManagementTab re-renders the tab from the backend's current preset list", async () => {
    invokeImplFor({
      list_config_presets: () => [{ id: "team-style", label: "Team Style", description: "" }],
    });

    await refreshPresetManagementTab();

    expect(document.querySelector("#tab-presets")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelectorAll("#preset-management-list .preset-management__item").length).toBe(1);
    expect(
      Array.from(document.querySelectorAll<HTMLOptionElement>("#reset-to-preset-select option")).map(
        (option) => option.value,
      ),
    ).toEqual(["team-style"]);
  });

  it("renameUserPreset invokes rename_user_preset with the given arguments", async () => {
    invokeImplFor({ rename_user_preset: () => undefined });

    await renameUserPreset("old-name", "new-name", true);

    expect(invokeMock).toHaveBeenCalledWith("rename_user_preset", {
      oldName: "old-name",
      newName: "new-name",
      overwrite: true,
    });
  });

  it("deleteUserPreset invokes delete_user_preset with the given name", async () => {
    invokeImplFor({ delete_user_preset: () => undefined });

    await deleteUserPreset("team-style");

    expect(invokeMock).toHaveBeenCalledWith("delete_user_preset", { name: "team-style" });
  });

  it("exportUserPreset invokes export_user_preset and returns its YAML", async () => {
    invokeImplFor({ export_user_preset: () => "semicolon: true\n" });

    await expect(exportUserPreset("team-style")).resolves.toBe("semicolon: true\n");
    expect(invokeMock).toHaveBeenCalledWith("export_user_preset", { name: "team-style" });
  });
});

describe("handleRenamePresetClick", () => {
  const preset = { id: "team-style", label: "Team Style", description: "" };

  // mountFixture's DOMContentLoaded dispatch (see the top-level beforeEach)
  // already fires a handful of startup calls, including
  // refreshPresetManagementTab's own list_config_presets lookup; clear
  // those so the "does nothing"-style assertions below (checking invokeMock
  // was never called at all) aren't tripped up by them.
  beforeEach(() => {
    invokeMock.mockClear();
  });

  it("does nothing when the prompt is left blank", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("   ");

    await handleRenamePresetClick(preset);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing when the new name is unchanged, ignoring case", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("TEAM-STYLE");

    await handleRenamePresetClick(preset);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("renames without asking to overwrite when the new name is unused", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("new-name");
    const confirmSpy = vi.spyOn(window, "confirm");
    invokeImplFor({
      list_config_presets: () => [preset],
      rename_user_preset: () => undefined,
    });

    await handleRenamePresetClick(preset);

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("rename_user_preset", {
      oldName: "team-style",
      newName: "new-name",
      overwrite: false,
    });
  });

  it("asks to overwrite when the new name is already used, and cancels if declined", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("other-preset");
    vi.spyOn(window, "confirm").mockReturnValue(false);
    invokeImplFor({
      list_config_presets: () => [preset, { id: "other-preset", label: "Other Preset", description: "" }],
    });

    await handleRenamePresetClick(preset);

    expect(window.confirm).toHaveBeenCalledWith('A preset named "other-preset" already exists. Overwrite it?');
    expect(invokeMock).not.toHaveBeenCalledWith("rename_user_preset", expect.anything());
  });

  it("overwrites when confirmed and refreshes the tab", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("other-preset");
    vi.spyOn(window, "confirm").mockReturnValue(true);
    invokeImplFor({
      list_config_presets: () => [preset, { id: "other-preset", label: "Other Preset", description: "" }],
      rename_user_preset: () => undefined,
    });

    await handleRenamePresetClick(preset);

    expect(invokeMock).toHaveBeenCalledWith("rename_user_preset", {
      oldName: "team-style",
      newName: "other-preset",
      overwrite: true,
    });
  });

  it("alerts on failure", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("new-name");
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      list_config_presets: () => [preset],
      rename_user_preset: () => {
        throw new Error("disk full");
      },
    });

    await handleRenamePresetClick(preset);

    expect(window.alert).toHaveBeenCalledWith('Failed to rename preset "Team Style": Error: disk full');
  });
});

describe("handleDeletePresetClick", () => {
  const preset = { id: "team-style", label: "Team Style", description: "" };

  beforeEach(() => {
    invokeMock.mockClear();
  });

  it("does nothing when the confirmation is declined", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);

    await handleDeletePresetClick(preset);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("deletes and refreshes the tab when confirmed", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    invokeImplFor({
      delete_user_preset: () => undefined,
      list_config_presets: () => [],
    });

    await handleDeletePresetClick(preset);

    expect(invokeMock).toHaveBeenCalledWith("delete_user_preset", { name: "team-style" });
  });

  it("alerts on failure", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      delete_user_preset: () => {
        throw new Error("permission denied");
      },
    });

    await handleDeletePresetClick(preset);

    expect(window.alert).toHaveBeenCalledWith('Failed to delete preset "Team Style": Error: permission denied');
  });
});

describe("handleExportPresetClick", () => {
  const preset = { id: "team-style", label: "Team Style", description: "" };

  it("downloads the preset's YAML content as <id>.yaml", async () => {
    invokeImplFor({ export_user_preset: () => "semicolon: true\n" });
    const objectUrl = "blob:mock-url";
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue(objectUrl);
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      expect(this.download).toBe("team-style.yaml");
    });

    await handleExportPresetClick(preset);

    expect(createObjectURL).toHaveBeenCalledTimes(1);
    const [blob] = createObjectURL.mock.calls[0] as [Blob];
    expect(blob.type).toBe("application/x-yaml");
    expect(click).toHaveBeenCalledTimes(1);
  });

  it("alerts on failure", async () => {
    vi.spyOn(window, "alert").mockImplementation(() => {});
    invokeImplFor({
      export_user_preset: () => {
        throw new Error("not found");
      },
    });

    await handleExportPresetClick(preset);

    expect(window.alert).toHaveBeenCalledWith('Failed to export preset "Team Style": Error: not found');
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
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
      { rule: "argument-types", description: "Test description for argument types.", kinds: ["correctness"], importance: "high", auto_fixable: false },
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

describe("populateRuleFilterGroups (via applyRuleTags)", () => {
  const sampleTags: RuleTagsInfo[] = [
    { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
    { rule: "argument-types", description: "Test description for argument types.", kinds: ["performance", "correctness"], importance: "high", auto_fixable: false },
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
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
      { rule: "comma-spacing", description: "Test description for comma spacing.", kinds: ["style"], importance: "low", auto_fixable: true },
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
    applyRuleTags([{ rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true }]);
    const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
    select.options[0].selected = false;
    select.dispatchEvent(new Event("change"));

    const header = document.querySelector<HTMLInputElement>("#filter-kind-style")!;
    header.checked = true;
    header.dispatchEvent(new Event("change"));

    expect(select.options[0].selected).toBe(true);
    expect(matchesTagFilters({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toBe(true);
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

  it("lint_psc_file forwards the currently configured compiler path and compile-check setting", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe",
      load_compile_check: () => true,
      load_script_roots: () => [],
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
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rule: "trailing-whitespace",
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
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
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
    } finally {
      applyRuleTags([]);
    }
  });

  it("omits the auto-fixable badge for a fixable rule's finding that its own message says can't be fixed", () => {
    applyRuleTags([{ rule: "type-casing", description: "Test description for type casing.", kinds: ["style"], importance: "low", auto_fixable: true }]);
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
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
      { rule: "comma-spacing", description: "Test description for comma spacing.", kinds: ["style"], importance: "low", auto_fixable: true },
    ]);
    try {
      const select = document.querySelector<HTMLSelectElement>("#filter-rule-style")!;
      const trailingWhitespaceOption = [...select.options].find((option) => option.value === "trailing-whitespace")!;
      trailingWhitespaceOption.selected = false;
      select.dispatchEvent(new Event("change"));

      const item = buildPscResultItem(
        outcome({
          findings: [
            { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
            { line: 2, column: 1, message: "[warning] missing space", rule: "comma-spacing" },
          ],
        }),
      );
      const findingEls = item!.querySelectorAll(".psc-result__finding");
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

  it("renderPscResults respects the active tag filters", () => {
    applyRuleTags([{ rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true }]);
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

describe("massFixRuleDisplayName", () => {
  it("returns the human-readable name for a known fixable rule", () => {
    expect(massFixRuleDisplayName("trailing-whitespace")).toBe("Trailing whitespace");
    expect(massFixRuleDisplayName("comma-spacing")).toBe("Space after comma");
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

describe("collectFilteredIssues", () => {
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
      "scripts/source/A.psc:1:1: [trailing-whitespace] [warning] trailing whitespace\n" +
        "scripts/source/A.psc:5:3: [forbidden-functions] [error] forbidden function used",
    );
  });

  it("falls back to 'unknown' for a finding with no rule id", () => {
    const text = formatIssuesAsText([
      { path: "A.psc", findings: [{ line: 1, column: 1, message: "[error] compiler failure" }] },
    ]);

    expect(text).toBe("A.psc:1:1: [unknown] [error] compiler failure");
  });

  it("returns an empty string for no files", () => {
    expect(formatIssuesAsText([])).toBe("");
  });
});

describe("formatIssuesAsJson", () => {
  it("mirrors the CLI --json report shape, restricted to the given files/findings", () => {
    const json = formatIssuesAsJson([
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
      {
        path: "B.psc",
        findings: [
          { line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" },
          { line: 6, column: 1, message: "[info] consider renaming" },
        ],
      },
    ]);

    expect(JSON.parse(json)).toEqual({
      files: [
        {
          path: "A.psc",
          diagnostics: [
            { line: 1, column: 1, rule: "trailing-whitespace", level: "warning", message: "[warning] trailing whitespace" },
          ],
        },
        {
          path: "B.psc",
          diagnostics: [
            { line: 5, column: 3, rule: "forbidden-functions", level: "error", message: "[error] forbidden function used" },
            { line: 6, column: 1, rule: "unknown", level: "info", message: "[info] consider renaming" },
          ],
        },
      ],
      files_with_diagnostics: 2,
      total_diagnostics: 3,
    });
  });

  it("returns an empty report for no files", () => {
    expect(JSON.parse(formatIssuesAsJson([]))).toEqual({
      files: [],
      files_with_diagnostics: 0,
      total_diagnostics: 0,
    });
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
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
      { rule: "forbidden-functions", description: "Test description for forbidden functions.", kinds: ["performance", "correctness"], importance: "medium", auto_fixable: false },
    ]);
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

    const withSourceOmitted = JSON.parse(await formatIssuesForAi(files, "1.2.3"));
    expect(withSourceOmitted).toEqual({
      header: {
        tool: "Papyrus Lint",
        version: "1.2.3",
        website: "https://papyrus-lint.idrinth.de",
        target_game: "Skyrim SE/AE",
        generated_at: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/),
      },
      findings: {
        files: [
          {
            path: "A.psc",
            diagnostics: [
              { line: 1, column: 1, rule: "trailing-whitespace", level: "warning", message: "trailing whitespace" },
            ],
            source: null,
          },
          {
            path: "B.psc",
            diagnostics: [
              { line: 5, column: 3, rule: "forbidden-functions", level: "error", message: "forbidden function used" },
            ],
            source: null,
          },
        ],
        files_with_diagnostics: 2,
        total_diagnostics: 2,
      },
      rule_details: [
        { rule: "forbidden-functions", description: "Test description for forbidden functions.", kinds: ["performance", "correctness"], importance: "medium", auto_fixable: false },
        { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true },
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
    const sources = new Map([["A.psc", "ScriptName A\n"]]);

    const json = JSON.parse(await formatIssuesForAi(files, "1.0.0", sources));

    expect(json.findings.files).toEqual([
      expect.objectContaining({ path: "A.psc", source: "ScriptName A\n" }),
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
    expect(contents.findings.files).toEqual([expect.objectContaining({ source: "ScriptName A\n" })]);
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
    expect(contents.findings.files[0].source).toBe("<failed to read file: Error: boom>");
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

  it("shows the Apply fixes button when the loaded file has a fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => "line one  \n" });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    expect(document.querySelector<HTMLButtonElement>("#code-viewer-fix")!.hidden).toBe(false);
  });

  it("keeps the Apply fixes button hidden when the loaded file has no fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] forbidden function used" }]);

    expect(document.querySelector<HTMLButtonElement>("#code-viewer-fix")!.hidden).toBe(true);
  });

  it("shows the Preview fixes button when the loaded file has a fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => "line one  \n" });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    expect(document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!.hidden).toBe(false);
  });

  it("keeps the Preview fixes button hidden when the loaded file has no fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] forbidden function used" }]);

    expect(document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!.hidden).toBe(true);
  });
});

describe("handleCodeViewerFixClick", () => {
  async function openWithFixableFinding() {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
  }

  it("disables the button, repairs the file, and re-renders the viewer with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi.fn().mockResolvedValueOnce("line one  \n").mockResolvedValueOnce("line one\n"),
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;

    const promise = handleCodeViewerFixClick();
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.objectContaining({ path: "/a.psc" }));
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
    expect(button.disabled).toBe(false);
  });

  it("hides the button once nothing is left to fix", async () => {
    await openWithFixableFinding();
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;
    expect(button.hidden).toBe(false);

    await handleCodeViewerFixClick();

    expect(button.hidden).toBe(true);
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    // A failed read leaves codeViewerState null (openCodeViewer resets it to
    // null up front and only repopulates it after a successful read).
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();

    await handleCodeViewerFixClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and leaves the viewer untouched when the repair fails", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_file: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerFixClick();

    expect(button.disabled).toBe(false);
    expect(button.hidden).toBe(false);
  });
});

describe("handleCodeViewerPreviewFixClick", () => {
  it("disables the button, computes the diff, and renders it without touching the file", async () => {
    const diff = "--- /a.psc\n+++ /a.psc\n@@ -1,1 +1,1 @@\n-line one  \n+line one\n";
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      preview_repair_psc_file: () => diff,
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!;

    const promise = handleCodeViewerPreviewFixClick();
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith("preview_repair_psc_file", expect.objectContaining({ path: "/a.psc" }));
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("-line one  ");
    expect(outputEl.textContent).toContain("+line one");
    expect(button.disabled).toBe(false);
    // Nothing was written and the finding is still open, unlike a real fix.
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(1);
  });

  it("reports that nothing would change when the fix produces an empty diff", async () => {
    invokeImplFor({
      read_psc_file: () => "ScriptName Example\n",
      preview_repair_psc_file: () => "",
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    await handleCodeViewerPreviewFixClick();

    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toBe("No changes would be made.");
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();

    await handleCodeViewerPreviewFixClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and shows an error when computing the preview fails", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      preview_repair_psc_file: () => Promise.reject(new Error("parse error")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!;
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerPreviewFixClick();

    expect(button.disabled).toBe(false);
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("parse error");
    expect(outputEl.classList.contains("code-viewer__diff-output--error")).toBe(true);
  });

  it("clears a stale preview once a real fix is applied", async () => {
    invokeImplFor({
      read_psc_file: vi.fn().mockResolvedValueOnce("line one  \n").mockResolvedValueOnce("line one\n"),
      preview_repair_psc_file: () => "--- /a.psc\n+++ /a.psc\n@@ -1,1 +1,1 @@\n-line one  \n+line one\n",
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    await handleCodeViewerPreviewFixClick();
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);

    await handleCodeViewerFixClick();

    expect(outputEl.hidden).toBe(true);
  });
});

describe("code viewer edit mode", () => {
  async function openWithSource(source: string, findings: Diagnostic[] = []) {
    invokeImplFor({ read_psc_file: () => source });
    await openCodeViewer("/a.psc", findings);
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

    it("keeps linter severities visible in the editor and describes every finding in the accessible label", async () => {
      await openWithSource("line one\nline two\n", [
        { line: 2, column: 3, message: "[warning] risky edit" },
        { line: 2, column: 5, message: "[error] broken edit" },
      ]);

      enterCodeViewerEditMode();

      const lines = highlightCode().querySelectorAll(".code-viewer__editor-line");
      expect(lines).toHaveLength(3);
      expect(lines[1].classList.contains("code-viewer__line--error")).toBe(true);
      // The highlight layer's own per-line title would never actually be
      // hoverable (the textarea on top of it intercepts every pointer
      // event), so it carries no title of its own - see the
      // "hovered line" tooltip test below for how the textarea's title is
      // kept in sync instead.
      expect(lines[1].hasAttribute("title")).toBe(false);
      expect(textarea().getAttribute("aria-label")).toContain("Line 2, column 3: [warning] risky edit");
      expect(textarea().getAttribute("aria-label")).toContain("Line 2, column 5: [error] broken edit");
    });

    it("renders one line number per source line in the gutter, kept in sync as the user types", async () => {
      await openWithSource("line one\nline two\n");
      enterCodeViewerEditMode();

      const gutterLines = () =>
        Array.from(document.querySelectorAll("#code-viewer-editor-gutter .code-viewer__editor-gutter-line")).map(
          (el) => el.textContent,
        );
      expect(gutterLines()).toEqual(["1", "2", "3"]);

      textarea().value = "line one\nline two\nline three\n";
      textarea().dispatchEvent(new Event("input"));

      expect(gutterLines()).toEqual(["1", "2", "3", "4"]);
    });

    it("updates the textarea's tooltip to only the hovered line's findings, not the whole file's", async () => {
      await openWithSource("line one\nline two\nline three\n", [
        { line: 1, column: 1, message: "[info] first line" },
        { line: 3, column: 1, message: "[warning] third line" },
      ]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");
      expect(ta.title).not.toContain("[warning] third line");

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 55 }));
      expect(ta.title).toContain("[warning] third line");
      expect(ta.title).not.toContain("[info] first line");

      ta.dispatchEvent(new MouseEvent("mouseleave"));
      expect(ta.title).toBe("");
    });

    it("re-evaluates the tooltip on scroll, so a stationary pointer over a newly scrolled-in line isn't left describing the old one", async () => {
      await openWithSource("line one\nline two\nline three\n", [
        { line: 1, column: 1, message: "[info] first line" },
        { line: 3, column: 1, message: "[warning] third line" },
      ]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      const computedStyle = vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      // Hover line 1 while unscrolled.
      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");

      // Scroll line 3 underneath that same, still-stationary pointer
      // position (scrollTop of 40px shifts offsetY from 0 to 40, i.e. line
      // 3 under a mock that reports no scroll of its own).
      Object.defineProperty(ta, "scrollTop", { value: 40, configurable: true });
      ta.dispatchEvent(new Event("scroll"));

      expect(ta.title).toContain("[warning] third line");
      expect(ta.title).not.toContain("[info] first line");

      computedStyle.mockRestore();
    });

    it("clears the tooltip on scroll once the mouse has already left the editor, instead of reusing a stale position", async () => {
      await openWithSource("line one\nline two\nline three\n", [{ line: 1, column: 1, message: "[info] first line" }]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");

      ta.dispatchEvent(new MouseEvent("mouseleave"));
      ta.dispatchEvent(new Event("scroll"));

      expect(ta.title).toBe("");
    });

    it("renders each blank source line as its own line element with no stray whitespace between line elements", async () => {
      // Regression test: the highlight overlay's line spans are `display:
      // block` (styles.css), so a literal "\n" joining them used to render,
      // under `white-space: pre`, as an extra blank line stacked on top of
      // each blank source line's own (empty, so zero-height) span - visibly
      // desyncing the overlay's blank lines from the textarea's underneath
      // it. The overlay must instead emit exactly one line element per
      // source line, back to back, with nothing textual between them.
      await openWithSource("ScriptName Foo\n\nFunction Bar()\nEndFunction\n");

      enterCodeViewerEditMode();

      const lines = Array.from(highlightCode().querySelectorAll(".code-viewer__editor-line"));
      expect(lines).toHaveLength(5);
      expect(lines.map((line) => line.textContent)).toEqual(["ScriptName Foo", "", "Function Bar()", "EndFunction", ""]);
      for (let i = 0; i < lines.length - 1; i++) {
        expect(lines[i].nextSibling).toBe(lines[i + 1]);
      }
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

    it("is false right after entering edit mode on a CRLF-saved file", async () => {
      // A textarea's value getter normalizes CRLF to LF even though nothing
      // was typed, so comparing it against the CRLF source verbatim would
      // read as dirty with no edit having happened.
      await openWithSource("Int x = 1\r\nInt y = 2\r\n");
      enterCodeViewerEditMode();

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

    it("returns to view mode without confirming on a CRLF-saved file with no unsaved changes", async () => {
      await openWithSource("Int x = 1\r\nInt y = 2\r\n");
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
        load_compile_check: () => false,
        load_script_roots: () => [],
        parse_psc_file: () => ({ name: "A" }),
        lint_psc_file: () => [],
      });
      const pending = handleDroppedPaths(["/proj/list.achlist"]);
      await confirmDetectedConfig();
      await pending;
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
      vi.useFakeTimers();
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

      vi.advanceTimersByTime(2000);
      expect(saveButton.textContent).toBe("Save");
      vi.useRealTimers();
    });
  });

  describe("saveAndCompileCodeViewerEdits", () => {
    function compileOutputEl() {
      return document.querySelector<HTMLElement>("#code-viewer-compile-output")!;
    }

    it("saves the file and shows the compiler's output", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.\n", stderr: "" }),
      });

      await saveAndCompileCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", { path: "/a.psc", contents: "Int x = 2\n" });
      expect(invokeMock).toHaveBeenCalledWith("compile_psc_file", expect.objectContaining({ path: "/a.psc" }));
      expect(compileOutputEl().hidden).toBe(false);
      expect(compileOutputEl().textContent).toContain("Compilation succeeded.");
      expect(compileOutputEl().classList.contains("psc-result__compile-output--ok")).toBe(true);
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("marks a compiler-reported failure", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: false, stdout: "", stderr: "Broken.psc(3,1): error\n" }),
      });
      const button = document.querySelector<HTMLButtonElement>("#code-viewer-save-compile")!;

      await saveAndCompileCodeViewerEdits();

      expect(compileOutputEl().textContent).toContain("Broken.psc(3,1): error");
      expect(compileOutputEl().classList.contains("psc-result__compile-output--error")).toBe(true);
      expect(button.disabled).toBe(false);
      expect(button.textContent).toBe("Save & Compile");
    });

    it("does not compile, and reports a failure, when saving fails", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeMock.mockRejectedValue(new Error("disk full"));
      vi.spyOn(console, "error").mockImplementation(() => {});
      const button = document.querySelector<HTMLButtonElement>("#code-viewer-save-compile")!;

      await saveAndCompileCodeViewerEdits();

      expect(button.textContent).toBe("Save failed");
      expect(button.disabled).toBe(false);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
      expect(invokeMock).not.toHaveBeenCalledWith("compile_psc_file", expect.anything());
      expect(compileOutputEl().hidden).toBe(true);

      vi.advanceTimersByTime(2000);
      expect(button.textContent).toBe("Save & Compile");
      vi.useRealTimers();
    });

    it("clears a previous compile result when editing starts again", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: true, stdout: "ok", stderr: "" }),
      });
      await saveAndCompileCodeViewerEdits();
      expect(compileOutputEl().hidden).toBe(false);

      enterCodeViewerEditMode();

      expect(compileOutputEl().hidden).toBe(true);
      expect(compileOutputEl().textContent).toBe("");
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

    it("hides the dropdown when text is selected", async () => {
      const field = await openWithCursorAfterSelfDot();
      field.setSelectionRange(0, 4);

      await updateAutocomplete();

      expect(invokeMock).not.toHaveBeenCalledWith("list_script_members", expect.anything());
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("hides and clears the dropdown when the backend has no matching members", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({ list_script_members: () => [] });

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().children).toHaveLength(0);
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

    it("wraps upward, accepts with Tab, and ignores unrelated keys", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } },
          { kind: "property", name: "BProp", type_name: { name: "Int", is_array: false } },
        ],
      });
      await updateAutocomplete();

      const unrelated = new KeyboardEvent("keydown", { key: "Shift", cancelable: true });
      handleAutocompleteKeydown(unrelated);
      expect(unrelated.defaultPrevented).toBe(false);

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "ArrowUp", cancelable: true }));
      expect(autocompleteEl().querySelector(".code-viewer__autocomplete-item--active")?.textContent).toContain(
        "BProp",
      );
      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));
      expect(field.value).toContain("self.BProp");
    });

    it("leaves Tab to the dropdown instead of also inserting a literal tab", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      const event = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
      handleAutocompleteKeydown(event);
      handleEditorTabKeydown(event);

      expect(field.value).toContain("self.AProp");
      expect(field.value).not.toContain("\t");
    });

    it("accepts a completion when its dropdown item is clicked", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "Target", type_name: { name: "ObjectReference", is_array: false } },
        ],
      });
      await updateAutocomplete();

      autocompleteEl()
        .querySelector<HTMLButtonElement>(".code-viewer__autocomplete-item")!
        .dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));

      expect(field.value).toContain("self.Target");
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("ignores selection and navigation requests when no completion is available", () => {
      applyAutocompleteSelection(99);
      const event = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
      handleAutocompleteKeydown(event);
      expect(event.defaultPrevented).toBe(false);
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

    it("does not reopen the dropdown when a hidden lookup finishes", async () => {
      await openWithCursorAfterSelfDot();
      let finishLookup!: (members: unknown[]) => void;
      invokeImplFor({
        list_script_members: () =>
          new Promise<unknown[]>((resolve) => {
            finishLookup = resolve;
          }),
      });
      const pendingUpdate = updateAutocomplete();

      hideAutocomplete();
      finishLookup([{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }]);
      await pendingUpdate;

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().children).toHaveLength(0);
    });

    it("keeps the newest results when an older lookup finishes last", async () => {
      const field = await openWithCursorAfterSelfDot();
      const finishes: Array<(members: unknown[]) => void> = [];
      invokeImplFor({
        list_script_members: () =>
          new Promise<unknown[]>((resolve) => {
            finishes.push(resolve);
          }),
      });
      const olderUpdate = updateAutocomplete();
      field.setRangeText("b", field.selectionStart, field.selectionEnd, "end");
      const newerUpdate = updateAutocomplete();

      finishes[1]([{ kind: "property", name: "Better", type_name: { name: "Int", is_array: false } }]);
      await newerUpdate;
      finishes[0]([{ kind: "property", name: "Ancient", type_name: { name: "Int", is_array: false } }]);
      await olderUpdate;

      expect(autocompleteEl().textContent).toContain("Better");
      expect(autocompleteEl().textContent).not.toContain("Ancient");
    });

    it("listScriptMembers logs and returns an empty list when the backend call fails", async () => {
      invokeMock.mockRejectedValue(new Error("lookup failed"));
      vi.spyOn(console, "error").mockImplementation(() => {});

      await expect(listScriptMembers("Example")).resolves.toEqual([]);
    });
  });

  describe("handleEditorTabKeydown", () => {
    it("inserts a literal tab at the caret instead of letting focus leave the textarea", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
      handleEditorTabKeydown(event);

      expect(event.defaultPrevented).toBe(true);
      expect(field.value).toBe("Int\t x = 1\n");
      expect(field.selectionStart).toBe(4);
    });

    it("replaces the current selection with a tab", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(0, 3);

      handleEditorTabKeydown(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));

      expect(field.value).toBe("\t x = 1\n");
    });

    it("ignores keys other than Tab", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
      handleEditorTabKeydown(event);

      expect(event.defaultPrevented).toBe(false);
      expect(field.value).toBe("Int x = 1\n");
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

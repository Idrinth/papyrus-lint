import { describe, expect, it, vi } from "vitest";
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

import { invokeImplFor, loadProjectConfigConfirmed } from "./test/harness";
import { DEFAULT_LINT_CONFIG, type LintConfig } from "./config-types";
import { applyLookupScriptRootsToUI, applyProjectInfoToUI, applyScriptRootsToUI, handleCompileCheckChanged, handleCompilerPathChanged, handleConfigPathOverrideChanged, handleLookupScriptRootsChanged, handleScriptRootsChanged, loadProjectConfig, lookupScriptRootsFromUI, scriptRootsFromUI, setSettingsLocked, useProjectDir } from "./project-settings";
import { configPathOverride } from "./project-state";
import { loadCompileCheck, loadCompilerPath, loadLookupScriptRoots, loadProjectInfo, loadScriptRoots, projectDirForAchlist, projectDirForDirectory, projectDirForPscPath, saveCompileCheck, saveCompilerPath, saveLookupScriptRoots, saveScriptRoots } from "./project-io";
describe("projectDirForAchlist / projectDirForDirectory / projectDirForPscPath", () => {
  it("projectDirForAchlist asks the backend with only the .psc entries and the achlist's own directory as fallback", async () => {
    invokeImplFor({ find_project_root: () => "/proj/somefolder/otherfolder" });

    await expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/readme.txt",
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
      ]),
    ).resolves.toBe("/proj/somefolder/otherfolder");
    expect(invokeMock).toHaveBeenCalledWith("find_project_root", {
      entries: ["/proj/somefolder/otherfolder/scripts/source/AType.psc"],
      fallback: "/proj",
    });
  });

  it("projectDirForAchlist falls back to the achlist's own directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(projectDirForAchlist("/proj/list.achlist", ["/proj/other/A.psc"])).resolves.toBe("/proj");
  });

  it("projectDirForDirectory asks the backend with every entry and the dropped directory as fallback", async () => {
    invokeImplFor({ find_project_root: () => "/proj" });

    await expect(
      projectDirForDirectory("/proj/scripts/source", ["/proj/scripts/source/Requiem/A.psc"]),
    ).resolves.toBe("/proj");
    expect(invokeMock).toHaveBeenCalledWith("find_project_root", {
      entries: ["/proj/scripts/source/Requiem/A.psc"],
      fallback: "/proj/scripts/source",
    });
  });

  it("projectDirForDirectory falls back to the dropped directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(projectDirForDirectory("/proj", ["/proj/Nested/A.psc"])).resolves.toBe("/proj");
  });

  it("projectDirForPscPath asks the backend for the bare .psc file's project root", async () => {
    invokeImplFor({ find_psc_project_root_for_path: () => "/proj" });

    await expect(projectDirForPscPath("/proj/scripts/source/User/A.psc")).resolves.toBe("/proj");
    expect(invokeMock).toHaveBeenCalledWith("find_psc_project_root_for_path", {
      path: "/proj/scripts/source/User/A.psc",
    });
  });

  it("projectDirForPscPath falls back to two directories above the script's own directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(projectDirForPscPath("/proj/scripts/source/A.psc")).resolves.toBe("/proj");
  });
});

describe("project settings handlers", () => {
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

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
import {
  applyLookupScriptRootsToUI,
  applyScriptRootsToUI,
  handleCompileCheckChanged,
  handleCompilerPathChanged,
  handleConfigPathOverrideChanged,
  handleLookupScriptRootsChanged,
  handleScriptRootsChanged,
  loadProjectConfig,
  lookupScriptRootsFromUI,
  scriptRootsFromUI,
  setSettingsLocked,
  useProjectDir,
} from "./project-settings";
import { configPathOverride } from "./project-state";

describe("project settings handlers", () => {
  it("handleCompilerPathChanged persists the path once a project dir is known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLInputElement>("#compiler-path")!.value =
      "C:\\Tools\\PapyrusCompiler.exe";
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

    expect(invokeMock).toHaveBeenCalledWith("save_compile_check", {
      dir: "/proj",
      enabled: true,
    });
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

    document.querySelector<HTMLInputElement>("#config-path-override")!.value =
      "  /profiles/strict.yaml  ";
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
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
    });
    document.querySelector<HTMLInputElement>("#config-path-override")!.value =
      "/profiles/strict.yaml";
    handleConfigPathOverrideChanged();

    await vi.waitFor(() =>
      expect(
        document.querySelector<HTMLSelectElement>("#semicolon-style")!.value,
      ).toBe("require"),
    );
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", {
      path: "/profiles/strict.yaml",
    });
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("/profiles/strict.yaml");
  });
});

describe("scriptRootsFromUI / applyScriptRootsToUI", () => {
  it("scriptRootsFromUI splits non-blank lines and trims whitespace", () => {
    document.querySelector<HTMLTextAreaElement>("#script-roots")!.value =
      "  ../SharedScripts  \n\n/abs/OtherScripts\n";

    expect(scriptRootsFromUI()).toEqual([
      "../SharedScripts",
      "/abs/OtherScripts",
    ]);
  });

  it("scriptRootsFromUI returns an empty array for a blank textarea", () => {
    document.querySelector<HTMLTextAreaElement>("#script-roots")!.value =
      "   \n  \n";

    expect(scriptRootsFromUI()).toEqual([]);
  });

  it("applyScriptRootsToUI joins roots with newlines", () => {
    applyScriptRootsToUI(["../SharedScripts", "/abs/OtherScripts"]);

    expect(
      document.querySelector<HTMLTextAreaElement>("#script-roots")!.value,
    ).toBe("../SharedScripts\n/abs/OtherScripts");
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
    document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!.value =
      "   \n  \n";

    expect(lookupScriptRootsFromUI()).toEqual([]);
  });

  it("applyLookupScriptRootsToUI joins roots with newlines", () => {
    applyLookupScriptRootsToUI([
      "C:/Skyrim/Data/Scripts/Source",
      "C:/Skyrim/Data/Source/Scripts",
    ]);

    expect(
      document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!
        .value,
    ).toBe("C:/Skyrim/Data/Scripts/Source\nC:/Skyrim/Data/Source/Scripts");
  });
});

describe("useProjectDir", () => {
  it("loads the config and applies it to the UI", async () => {
    const custom: LintConfig = {
      ...DEFAULT_LINT_CONFIG,
      semicolon: true,
      indentation: "space",
    };
    invokeImplFor({
      load_lint_config: () => custom,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(
      document.querySelector<HTMLSelectElement>("#semicolon-style")!.value,
    ).toBe("require");
    expect(
      document.querySelector<HTMLSelectElement>("#indentation-style")!.value,
    ).toBe("spaces");
  });

  it("populates the compiler path input from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () =>
        "C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe",
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(
      document.querySelector<HTMLInputElement>("#compiler-path")!.value,
    ).toBe("C:\\Games\\Skyrim\\Papyrus Compiler\\PapyrusCompiler.exe");
  });

  it("populates the compile-check checkbox from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => true,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(
      document.querySelector<HTMLInputElement>("#compile-check")!.checked,
    ).toBe(true);
  });

  it("populates the script roots textarea from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => ["../SharedScripts", "/abs/OtherScripts"],
    });

    await useProjectDir("/my/project");

    expect(
      document.querySelector<HTMLTextAreaElement>("#script-roots")!.value,
    ).toBe("../SharedScripts\n/abs/OtherScripts");
  });

  it("populates the lookup script roots textarea from the backend", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_lookup_script_roots: () => [
        "C:/Skyrim/Data/Scripts/Source",
        "C:/Skyrim/Data/Source/Scripts",
      ],
    });

    await useProjectDir("/my/project");

    expect(
      document.querySelector<HTMLTextAreaElement>("#lookup-script-roots")!
        .value,
    ).toBe("C:/Skyrim/Data/Scripts/Source\nC:/Skyrim/Data/Source/Scripts");
  });

  it("shows detected script roots and the configuration file", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_script_roots: () => [],
      load_project_info: () => ({
        detected_script_roots: [
          "/my/project/scripts/source",
          "/shared/scripts",
        ],
        used_configuration_file: "/my/project/papyrus-lint.yml",
      }),
    });

    await useProjectDir("/my/project");

    expect(document.querySelector("#detected-script-roots")!.textContent).toBe(
      "/my/project/scripts/source\n/shared/scripts",
    );
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("/my/project/papyrus-lint.yml");
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
    document.querySelector<HTMLInputElement>("#config-path-override")!.value =
      "/profiles/strict.yaml";

    await useProjectDir("/my/project");

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", {
      path: "/profiles/strict.yaml",
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "load_lint_config",
      expect.anything(),
    );
    expect(
      document.querySelector<HTMLSelectElement>("#semicolon-style")!.value,
    ).toBe("require");
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("/profiles/strict.yaml");
  });

  it("never shows the config-selection dialog on its own - that's loadProjectConfig's job", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await useProjectDir("/my/project");

    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(
      false,
    );
  });
});

describe("setSettingsLocked", () => {
  it("disables the settings fieldset and shows the locked notice when locked", () => {
    setSettingsLocked(true);

    expect(
      document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!
        .disabled,
    ).toBe(true);
    expect(
      document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden,
    ).toBe(false);
  });

  it("enables the settings fieldset and hides the locked notice when unlocked", () => {
    setSettingsLocked(true);

    setSettingsLocked(false);

    expect(
      document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!
        .disabled,
    ).toBe(false);
    expect(
      document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden,
    ).toBe(true);
  });
});

describe("loadProjectConfig", () => {
  it("locks the Settings tab while the picker is open and unlocks it once resolved", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector("#config-picker")!.hasAttribute("open"),
      ).toBe(true),
    );
    expect(
      document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!
        .disabled,
    ).toBe(true);
    expect(
      document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden,
    ).toBe(false);

    document
      .querySelector<HTMLButtonElement>("#config-picker-continue")!
      .click();
    await pending;

    expect(
      document.querySelector<HTMLFieldSetElement>("#settings-fieldset")!
        .disabled,
    ).toBe(false);
    expect(
      document.querySelector<HTMLElement>("#settings-locked-notice")!.hidden,
    ).toBe(true);
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
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", {
      dir: "/my/project",
    });
  });

  it("applies the chosen preset before loading the project's config", async () => {
    const presets = [
      { id: "strict", label: "Strict", description: "Catches everything." },
      { id: "careful", label: "Careful", description: "The quietest option." },
    ];
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      list_config_presets: () => presets,
      apply_config_preset: () => undefined,
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(
        document.querySelectorAll(
          "#config-picker-preset-list .config-picker__preset-option",
        ).length,
      ).toBe(2),
    );
    document
      .querySelectorAll<HTMLButtonElement>(
        "#config-picker-preset-list .config-picker__preset-option",
      )[1]
      .click();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("apply_config_preset", {
      dir: "/my/project",
      preset: "careful",
    });
  });

  it("writes the first-run picker's game into a new project", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      list_config_presets: () => [
        {
          id: "careful",
          label: "Careful",
          description: "The quietest option.",
        },
      ],
      apply_config_preset: () => undefined,
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => undefined,
    });

    const pending = loadProjectConfig("/fallout/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector(
          "#config-picker-preset-list .config-picker__preset-option",
        ),
      ).not.toBeNull(),
    );
    document.querySelector<HTMLSelectElement>(
      "#config-picker-game-select",
    )!.value = "fallout4";
    document
      .querySelector<HTMLButtonElement>(
        "#config-picker-preset-list .config-picker__preset-option",
      )!
      .click();
    await pending;

    expect(
      document.querySelector<HTMLSelectElement>("#game-select")!.value,
    ).toBe("fallout4");
    expect(invokeMock).toHaveBeenCalledWith("save_lint_config", {
      dir: "/fallout/project",
      config: expect.objectContaining({ game: "fallout4" }),
    });
  });

  it("does not overwrite an existing configuration's game from the picker", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: "/my/project/papyrus-lint.yaml",
      }),
      load_lint_config: () => ({ ...DEFAULT_LINT_CONFIG, game: "fallout4" }),
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => undefined,
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector("#config-picker")!.hasAttribute("open"),
      ).toBe(true),
    );
    expect(
      document.querySelector<HTMLElement>("#config-picker-game")!.hidden,
    ).toBe(true);
    document
      .querySelector<HTMLButtonElement>("#config-picker-continue")!
      .click();
    await pending;

    expect(
      document.querySelector<HTMLSelectElement>("#game-select")!.value,
    ).toBe("fallout4");
    expect(invokeMock).not.toHaveBeenCalledWith(
      "save_lint_config",
      expect.anything(),
    );
  });

  it("writes a config when a new project continues as Fallout 4", async () => {
    let saves = 0;
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file:
          saves > 0 ? "/fallout/project/papyrus-lint.yaml" : null,
      }),
      list_config_presets: () => [],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => {
        saves += 1;
      },
    });

    const pending = loadProjectConfig("/fallout/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector("#config-picker")!.hasAttribute("open"),
      ).toBe(true),
    );
    document.querySelector<HTMLSelectElement>(
      "#config-picker-game-select",
    )!.value = "fallout4";
    document
      .querySelector<HTMLButtonElement>("#config-picker-continue")!
      .click();
    await pending;

    expect(saves).toBe(1);
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("/fallout/project/papyrus-lint.yaml");
    expect(
      document.querySelector<HTMLSelectElement>("#game-select")!.value,
    ).toBe("fallout4");
  });

  it("does not write a config when a new project continues as Skyrim", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      list_config_presets: () => [],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => undefined,
    });

    const pending = loadProjectConfig("/skyrim/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector("#config-picker")!.hasAttribute("open"),
      ).toBe(true),
    );
    document
      .querySelector<HTMLButtonElement>("#config-picker-continue")!
      .click();
    await pending;

    expect(invokeMock).not.toHaveBeenCalledWith(
      "save_lint_config",
      expect.anything(),
    );
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("None (using defaults)");
  });

  it("passes a custom preset id to the backend without normalizing it", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      list_config_presets: () => [
        {
          id: "Team Conventions",
          label: "Team Conventions",
          description: "A custom preset.",
        },
      ],
      apply_config_preset: () => undefined,
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector(
          "#config-picker-preset-list .config-picker__preset-option",
        ),
      ).not.toBeNull(),
    );
    document
      .querySelector<HTMLButtonElement>(
        "#config-picker-preset-list .config-picker__preset-option",
      )!
      .click();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("apply_config_preset", {
      dir: "/my/project",
      preset: "Team Conventions",
    });
  });

  it("uses a manually specified configuration file", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      load_lint_config_from_path: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    const pending = loadProjectConfig("/my/project");
    await vi.waitFor(() =>
      expect(
        document.querySelector("#config-picker")!.hasAttribute("open"),
      ).toBe(true),
    );
    document.querySelector<HTMLInputElement>(
      "#config-picker-path-input",
    )!.value = "/profiles/strict.yaml";
    document
      .querySelector<HTMLButtonElement>("#config-picker-use-path")!
      .click();
    await pending;

    expect(configPathOverride()).toBe("/profiles/strict.yaml");
    expect(invokeMock).toHaveBeenCalledWith("load_lint_config_from_path", {
      path: "/profiles/strict.yaml",
    });
  });

  it("does not re-show the picker for a directory already confirmed this session", async () => {
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await loadProjectConfigConfirmed("/my/project");
    invokeMock.mockClear();
    invokeImplFor({
      load_project_info: () => ({
        detected_script_roots: [],
        used_configuration_file: null,
      }),
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });

    await loadProjectConfig("/my/project");

    expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(
      false,
    );
  });
});

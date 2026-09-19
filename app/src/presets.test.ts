import { beforeEach, describe, expect, it, vi } from "vitest";
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

import { invokeImplFor } from "./test/harness";
import { switchTab } from "./main-tabs";
import { type ConfigSelectionResult } from "./main-types";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { handleLintConfigChanged } from "./config-ui";
import { useProjectDir } from "./project-settings";
import { applyConfigPreset, deleteUserPreset, exportUserPreset, getPresetLintConfig, isCustomPreset, loadConfigPresets, renameUserPreset } from "./presets-api";
import { handleDeletePresetClick, handleExportPresetClick, handleRenamePresetClick, handleResetToPresetClick, handleSaveConfigAsPresetClick, populateResetPresetSelect, refreshPresetManagementTab, renderPresetManagementTab } from "./presets-management";
import { promptForConfigSelection } from "./presets-picker";
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

  it("wires each custom preset action button to its browser interaction", async () => {
    const prompt = vi.spyOn(window, "prompt").mockReturnValue(null);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const createObjectURL = vi.spyOn(URL, "createObjectURL").mockReturnValue("blob:preset");
    vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    invokeImplFor({ export_user_preset: () => "semicolon_style: require\n" });

    renderPresetManagementTab([
      ...builtIns,
      { id: "team-style", label: "Team Style", description: "Our house rules." },
    ]);
    const buttons = document.querySelectorAll<HTMLButtonElement>(".preset-management__button");

    buttons[0].click();
    buttons[1].click();
    buttons[2].click();

    expect(prompt).toHaveBeenCalledWith('Rename preset "Team Style" to:', "Team Style");
    expect(confirm).toHaveBeenCalledWith('Delete preset "Team Style"? This can\'t be undone.');
    await vi.waitFor(() => expect(createObjectURL).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledWith("export_user_preset", { name: "team-style" });
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

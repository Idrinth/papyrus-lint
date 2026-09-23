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

import { invokeImplFor } from "./test/harness";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { applyConfigPreset, deleteUserPreset, exportUserPreset, getPresetLintConfig, isCustomPreset, loadConfigPresets, renameUserPreset } from "./presets-api";
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

describe("getPresetLintConfig", () => {
  it("fetches the named preset's lint settings from the backend", async () => {
    const config = { ...DEFAULT_LINT_CONFIG, semicolon: false };
    invokeImplFor({ get_preset_lint_config: () => config });

    await expect(getPresetLintConfig("careful")).resolves.toEqual(config);
    expect(invokeMock).toHaveBeenCalledWith("get_preset_lint_config", { preset: "careful" });
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

describe("renameUserPreset / deleteUserPreset / exportUserPreset", () => {
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

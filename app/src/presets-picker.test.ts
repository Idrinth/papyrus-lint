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
import { type ConfigSelectionResult } from "./main-types";
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
    expect(document.querySelector<HTMLElement>("#config-picker-game")!.hidden).toBe(true);
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
    expect(document.querySelector<HTMLElement>("#config-picker-game")!.hidden).toBe(false);
    expect(document.querySelector<HTMLSelectElement>("#config-picker-game-select")!.value).toBe("skyrim");
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

    await expect(pending).resolves.toEqual({ kind: "preset", preset: "careful", game: "skyrim" });
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

    await expect(pending).resolves.toEqual({ kind: "preset", preset: "Team Conventions", game: "skyrim" });
  });

  it("includes the selected target game when a new project continues", async () => {
    const continued = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker")!.hasAttribute("open")).toBe(true),
    );
    document.querySelector<HTMLSelectElement>("#config-picker-game-select")!.value = "fallout4";
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
    await expect(continued).resolves.toEqual({ kind: "detected", game: "fallout4" });
  });

  it("includes the selected target game when a new project picks a preset", async () => {
    invokeImplFor({
      list_config_presets: () => [
        { id: "strict", label: "Strict", description: "Catches everything." },
      ],
    });

    const preset = promptForConfigSelection({ detected_script_roots: [], used_configuration_file: null });
    await vi.waitFor(() =>
      expect(document.querySelector("#config-picker-preset-list .config-picker__preset-option")).not.toBeNull(),
    );
    document.querySelector<HTMLSelectElement>("#config-picker-game-select")!.value = "fallout4";
    document.querySelector<HTMLButtonElement>("#config-picker-preset-list .config-picker__preset-option")!.click();
    await expect(preset).resolves.toEqual({ kind: "preset", preset: "strict", game: "fallout4" });
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

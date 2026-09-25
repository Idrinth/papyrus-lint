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
import { useProjectDir } from "./project-settings";
import { applyLintConfigToUI, configKeyForRuleId, disableRulesInLintConfig, handleLintConfigChanged, lintConfigFromUI } from "./config-ui";
import { DEFAULT_LINT_CONFIG, DEFAULT_RULES, type LintConfig, setCurrentLintConfig } from "./config-types";
import { loadLintConfig, loadLintConfigFromPath, saveLintConfig, saveLintConfigToPath } from "./config-io";
describe("lint config UI round trip", () => {
  it("applyLintConfigToUI followed by lintConfigFromUI reproduces the config", () => {
    const config: LintConfig = {
      game: "skyrim",
      semicolon: true,
      indentation: "space",
      indentation_width: 8,
      max_line_length: 100,
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

  it("round-trips a Fallout 4 project through the target-game control", () => {
    applyLintConfigToUI({ ...DEFAULT_LINT_CONFIG, game: "fallout4" });
    expect(document.querySelector<HTMLSelectElement>("#game-select")!.value).toBe("fallout4");
    expect(lintConfigFromUI().game).toBe("fallout4");

    document.querySelector<HTMLSelectElement>("#game-select")!.value = "skyrim";
    expect(lintConfigFromUI().game).toBe("skyrim");
  });

  it("keeps a starfield project instead of rewriting it to Skyrim", () => {
    const starfield = { ...DEFAULT_LINT_CONFIG, game: "starfield" as LintConfig["game"] };
    setCurrentLintConfig(starfield);
    applyLintConfigToUI(starfield);

    const gameSelect = document.querySelector<HTMLSelectElement>("#game-select")!;
    expect(gameSelect.value).toBe("starfield");
    // Starfield is a listed target game, so it uses the fixture option
    // rather than the extra option reserved for a game the picker does not offer.
    expect(gameSelect.selectedOptions[0]?.hasAttribute("data-unlisted-game")).toBe(false);
    expect(lintConfigFromUI().game).toBe("starfield");

    document.querySelector<HTMLSelectElement>("#semicolon-style")!.value = "require";
    expect(lintConfigFromUI().game).toBe("starfield");

    gameSelect.value = "fallout4";
    expect(lintConfigFromUI().game).toBe("fallout4");

    gameSelect.value = "starfield";
    expect(lintConfigFromUI().game).toBe("starfield");

    setCurrentLintConfig(DEFAULT_LINT_CONFIG);
    applyLintConfigToUI(DEFAULT_LINT_CONFIG);
  });

  it("applyLintConfigToUI enables the width field only for space indentation", () => {
    applyLintConfigToUI({ ...DEFAULT_LINT_CONFIG, indentation: "space" });
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(false);

    applyLintConfigToUI({ ...DEFAULT_LINT_CONFIG, indentation: "tab" });
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(true);
  });

  it("lintConfigFromUI clamps widths and complexity thresholds", () => {
    document.querySelector<HTMLInputElement>("#indentation-width")!.value = "100";
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-warning")!.value = "-5";
    document.querySelector<HTMLInputElement>("#cyclomatic-complexity-error")!.value = "-5";
    document.querySelector<HTMLInputElement>("#max-line-length")!.value = "-5";

    const config = lintConfigFromUI();
    expect(config.indentation_width).toBe(16);
    expect(config.max_line_length).toBe(1);
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

  it("handleLintConfigChanged persists a changed target game", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      save_lint_config: () => undefined,
    });
    await useProjectDir("/proj");
    invokeMock.mockClear();

    document.querySelector<HTMLSelectElement>("#game-select")!.value = "fallout4";
    handleLintConfigChanged();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("save_lint_config", {
      dir: "/proj",
      config: expect.objectContaining({ game: "fallout4" }),
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
});

describe("configKeyForRuleId / disableRulesInLintConfig", () => {
  it("maps hyphenated rule ids onto LintRules keys, including the two exceptions", () => {
    expect(configKeyForRuleId("comma-spacing")).toBe("comma_spacing");
    expect(configKeyForRuleId("float-to-int")).toBe("float_int_conversion");
    expect(configKeyForRuleId("too-many-named-states")).toBe("too_many_states");
    expect(configKeyForRuleId("compiler-error")).toBeUndefined();
  });

  it("disableRulesInLintConfig unchecks the matching Settings checkboxes", () => {
    expect(document.querySelector<HTMLInputElement>("#rule-comma_spacing")!.checked).toBe(true);

    expect(disableRulesInLintConfig(["comma-spacing", "compiler-error"])).toBe(true);

    expect(document.querySelector<HTMLInputElement>("#rule-comma_spacing")!.checked).toBe(false);
    expect(lintConfigFromUI().rules.comma_spacing).toBe(false);
  });

  it("disableRulesInLintConfig is a no-op when nothing configurable is still enabled", () => {
    expect(disableRulesInLintConfig(["compiler-error"])).toBe(false);
    expect(disableRulesInLintConfig(["comma-spacing"])).toBe(true);
    expect(disableRulesInLintConfig(["comma-spacing"])).toBe(false);
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

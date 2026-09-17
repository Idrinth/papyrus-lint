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

import { aiConfiguration } from "./results-export-ai";
import { DEFAULT_LINT_CONFIG, DEFAULT_RULES } from "./config";

describe("aiConfiguration", () => {
  it("replaces the rules object with a sorted list of just the enabled rule ids", () => {
    const config = { ...DEFAULT_LINT_CONFIG, rules: { ...DEFAULT_RULES, trailing_whitespace: false, property_sorting: true } };

    const result = aiConfiguration(config);

    expect(result.rules).toBeUndefined();
    expect(result.semicolon).toBe(DEFAULT_LINT_CONFIG.semicolon);
    expect(result.enabled_rules).not.toContain("trailing-whitespace");
    expect(result.enabled_rules).toContain("property-sorting");
    expect(result.enabled_rules).toContain("argument-types");
    expect(result.enabled_rules).toEqual([...(result.enabled_rules as string[])].sort());
  });
});

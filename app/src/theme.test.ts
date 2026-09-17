import { afterEach, describe, expect, it, vi } from "vitest";
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

import "./test/harness";
import { mountFixture } from "./test/fixture";
import { applyTheme, loadStoredTheme, storeTheme } from "./theme";

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

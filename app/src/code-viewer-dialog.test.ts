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
import { enterCodeViewerEditMode } from "./live-edit-persist";
import { openCodeViewer, requestCloseCodeViewer, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
import { setCurrentProjectDir } from "./project-state";

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

  it("shows the path relative to the current project dir, when known", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")' });
    setCurrentProjectDir("/proj");
    try {
      await openCodeViewer("/proj/scripts/source/A.psc", []);
      expect(document.querySelector("#code-viewer-title")!.textContent).toBe("scripts/source/A.psc");
    } finally {
      setCurrentProjectDir(null);
    }
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

  it("shows a per-line Fix, Ignore, File disable, and Config disable button for a fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => "line one  \n" });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="fix"]')).not.toBeNull();
    expect(row.querySelector('[data-line-action="ignore"]')).not.toBeNull();
    expect(row.querySelector('[data-line-action="file-disable"]')).not.toBeNull();
    expect(row.querySelector('[data-line-action="config-disable"]')).not.toBeNull();
  });

  it("shows Ignore, File disable, and Config disable for a rule with no automatic fix", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" }]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="fix"]')).toBeNull();
    expect(row.querySelector('[data-line-action="ignore"]')).not.toBeNull();
    expect(row.querySelector('[data-line-action="file-disable"]')).not.toBeNull();
    expect(row.querySelector('[data-line-action="config-disable"]')).not.toBeNull();
  });

  it("shows neither per-line button for a finding with no rule id", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });

    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] compiler error" }]);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="fix"]')).toBeNull();
    expect(row.querySelector('[data-line-action="ignore"]')).toBeNull();
    expect(row.querySelector('[data-line-action="file-disable"]')).toBeNull();
    expect(row.querySelector('[data-line-action="config-disable"]')).toBeNull();
  });

  it("shows no per-line buttons for a line with no findings", async () => {
    invokeImplFor({ read_psc_file: () => "line one\n" });

    await openCodeViewer("/a.psc", []);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector(".code-viewer__line-action")).toBeNull();
  });

  it("shows a per-line Nodiscard button on a function header that returns a value, with no finding needed", async () => {
    invokeImplFor({ read_psc_file: () => "Int Function GetValue()\n    Return 1\nEndFunction\n" });

    await openCodeViewer("/a.psc", []);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="nodiscard"]')).not.toBeNull();
  });

  it("shows a per-line Nodiscard button on a Native function with no return value", async () => {
    invokeImplFor({ read_psc_file: () => "Function DoThing() Native\n" });

    await openCodeViewer("/a.psc", []);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="nodiscard"]')).not.toBeNull();
  });

  it("does not show a per-line Nodiscard button on a void, non-native function", async () => {
    invokeImplFor({ read_psc_file: () => "Function DoThing()\nEndFunction\n" });

    await openCodeViewer("/a.psc", []);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="nodiscard"]')).toBeNull();
  });

  it("does not show a per-line Nodiscard button on a header already flagged @nodiscard", async () => {
    invokeImplFor({ read_psc_file: () => "Int Function GetValue() ; @nodiscard\n" });

    await openCodeViewer("/a.psc", []);

    const row = document.querySelector("#code-viewer-line-1")!;
    expect(row.querySelector('[data-line-action="nodiscard"]')).toBeNull();
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

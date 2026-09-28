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
import { openCodeViewer } from "./code-viewer-dialog";

describe("bindLiveEdit", () => {
  async function openEditableSource() {
    invokeImplFor({ read_psc_file: () => "ScriptName Example\n" });
    await openCodeViewer("/a.psc", []);
  }

  it("wires the Edit and Cancel buttons to the editor mode controls", async () => {
    await openEditableSource();
    const editor = document.querySelector<HTMLElement>("#code-viewer-editor")!;

    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();

    expect(editor.hidden).toBe(false);
    expect(document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!.value).toBe(
      "ScriptName Example\n",
    );

    document.querySelector<HTMLButtonElement>("#code-viewer-cancel")!.click();

    expect(editor.hidden).toBe(true);
  });

  it("keeps the syntax highlight and line-number gutter aligned while scrolling", async () => {
    await openEditableSource();
    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();
    const textarea = document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
    const highlight = document.querySelector<HTMLElement>("#code-viewer-editor-highlight")!;
    const gutter = document.querySelector<HTMLElement>("#code-viewer-editor-gutter")!;
    textarea.scrollTop = 42;
    textarea.scrollLeft = 17;

    textarea.dispatchEvent(new Event("scroll"));

    expect(highlight.scrollTop).toBe(42);
    expect(highlight.scrollLeft).toBe(17);
    expect(gutter.scrollTop).toBe(42);
  });

  it("wires Save to persist the edited source", async () => {
    await openEditableSource();
    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();
    const textarea = document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
    textarea.value = "ScriptName Updated\n";
    invokeImplFor({
      read_psc_file: () => "ScriptName Updated\n",
      write_psc_file: () => undefined,
      lint_psc_file: () => [],
    });

    document.querySelector<HTMLButtonElement>("#code-viewer-save")!.click();
    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "write_psc_file",
        expect.objectContaining({ path: "/a.psc", contents: "ScriptName Updated\n" }),
      );
      expect(document.querySelector<HTMLElement>("#code-viewer-editor")!.hidden).toBe(true);
    });
  });
});

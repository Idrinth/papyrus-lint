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
import { handleCodeViewerFixClick } from "./code-viewer-actions";
import { handleCodeViewerPreviewFixClick } from "./code-viewer-diff";
import { openCodeViewer } from "./code-viewer-dialog";

describe("handleCodeViewerPreviewFixClick", () => {
  it("disables the button, computes the diff, and renders it without touching the file", async () => {
    const diff = "--- /a.psc\n+++ /a.psc\n@@ -1,1 +1,1 @@\n-line one  \n+line one\n";
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      preview_repair_psc_file: () => diff,
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!;

    const promise = handleCodeViewerPreviewFixClick();
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith("preview_repair_psc_file", expect.objectContaining({ path: "/a.psc" }));
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("-line one  ");
    expect(outputEl.textContent).toContain("+line one");
    expect(button.disabled).toBe(false);
    // Nothing was written and the finding is still open, unlike a real fix.
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(1);
  });

  it("reports that nothing would change when the fix produces an empty diff", async () => {
    invokeImplFor({
      read_psc_file: () => "ScriptName Example\n",
      preview_repair_psc_file: () => "",
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    await handleCodeViewerPreviewFixClick();

    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toBe("No changes would be made.");
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();

    await handleCodeViewerPreviewFixClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and shows an error when computing the preview fails", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      preview_repair_psc_file: () => Promise.reject(new Error("parse error")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!;
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerPreviewFixClick();

    expect(button.disabled).toBe(false);
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("parse error");
    expect(outputEl.classList.contains("code-viewer__diff-output--error")).toBe(true);
  });

  it("clears a stale preview once a real fix is applied", async () => {
    invokeImplFor({
      read_psc_file: vi.fn().mockResolvedValueOnce("line one  \n").mockResolvedValueOnce("line one\n"),
      preview_repair_psc_file: () => "--- /a.psc\n+++ /a.psc\n@@ -1,1 +1,1 @@\n-line one  \n+line one\n",
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    await handleCodeViewerPreviewFixClick();
    const outputEl = document.querySelector<HTMLElement>("#code-viewer-diff-output")!;
    expect(outputEl.hidden).toBe(false);

    await handleCodeViewerFixClick();

    expect(outputEl.hidden).toBe(true);
  });
});

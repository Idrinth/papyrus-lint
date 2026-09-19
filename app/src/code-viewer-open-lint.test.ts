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
describe("openCodeViewer fresh lint", () => {
  it("re-lints the file on open so Ignore/Fix use current line numbers", async () => {
    invokeImplFor({
      read_psc_file: () => "Int a = 1\nInt b = 2  \n",
      lint_psc_file: () => [
        {
          line: 2,
          column: 12,
          message: "[warning] Line contains trailing whitespace",
          rule: "trailing-whitespace",
        },
      ],
    });

    await openCodeViewer("/a.psc", [
      {
        line: 1,
        column: 1,
        message: "[warning] Line contains trailing whitespace",
        rule: "trailing-whitespace",
      },
    ]);

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", expect.objectContaining({ path: "/a.psc" }));
    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      false,
    );
    expect(document.querySelector("#code-viewer-line-2")!.classList.contains("code-viewer__line--warning")).toBe(
      true,
    );
    expect(document.querySelector('#code-viewer-line-2 [data-line-action="ignore"]')).not.toBeNull();
  });
});

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
import { lintPapyrusScript, type Diagnostic } from "./backend";
import { openCodeViewer } from "./code-viewer-dialog";
import {
  cancelCodeViewerEditMode,
  enterCodeViewerEditMode,
} from "./live-edit-persist";

describe("code viewer edit mode", () => {
  async function openWithSource(source: string, findings: Diagnostic[] = []) {
    invokeImplFor({ read_psc_file: () => source });
    await openCodeViewer("/a.psc", findings);
  }

  function textarea() {
    return document.querySelector<HTMLTextAreaElement>(
      "#code-viewer-editor-textarea",
    )!;
  }

  function highlightCode() {
    return document.querySelector("#code-viewer-editor-highlight code")!;
  }

  describe("live linting while editing (scheduleLiveEditLint)", () => {
    it("lints the textarea's current contents via lint_papyrus_script after debouncing an edit", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      invokeImplFor({
        lint_papyrus_script: () => [
          { line: 1, column: 1, message: "[error] broken edit" },
        ],
      });

      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));
      await vi.advanceTimersByTimeAsync(400);

      expect(invokeMock).toHaveBeenCalledWith(
        "lint_papyrus_script",
        expect.objectContaining({ source: "Int x = 2\n" }),
      );
      expect(
        highlightCode().querySelectorAll(".code-viewer__line--error"),
      ).toHaveLength(1);
      vi.useRealTimers();
    });

    it("coalesces rapid successive edits into a single debounced lint", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      invokeImplFor({ lint_papyrus_script: () => [] });

      textarea().value = "Int x = 12\n";
      textarea().dispatchEvent(new Event("input"));
      await vi.advanceTimersByTimeAsync(100);
      textarea().value = "Int x = 123\n";
      textarea().dispatchEvent(new Event("input"));
      await vi.advanceTimersByTimeAsync(400);

      const liveLintCalls = invokeMock.mock.calls.filter(
        ([command]) => command === "lint_papyrus_script",
      );
      expect(liveLintCalls).toHaveLength(1);
      expect(liveLintCalls[0][1]).toMatchObject({ source: "Int x = 123\n" });
      vi.useRealTimers();
    });

    it("keeps the last saved findings visible until the first live lint resolves", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n", [
        { line: 1, column: 1, message: "[warning] stale" },
      ]);
      enterCodeViewerEditMode();

      expect(
        highlightCode().querySelectorAll(".code-viewer__line--warning"),
      ).toHaveLength(1);
      vi.useRealTimers();
    });

    it("discards a stale response once edit mode is left before it resolves", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      let resolveLint: (value: unknown) => void = () => {};
      invokeImplFor({
        lint_papyrus_script: () =>
          new Promise((resolve) => {
            resolveLint = resolve;
          }),
      });

      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));
      await vi.advanceTimersByTimeAsync(400);
      vi.spyOn(window, "confirm").mockReturnValue(true);
      cancelCodeViewerEditMode();
      resolveLint([{ line: 1, column: 1, message: "[error] too late" }]);
      await Promise.resolve();
      await Promise.resolve();

      expect(
        document.querySelectorAll(
          "#code-viewer-editor-highlight .code-viewer__line--error",
        ),
      ).toHaveLength(0);
      vi.useRealTimers();
    });

    it("does not schedule a live lint once edit mode has already been left", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));
      vi.spyOn(window, "confirm").mockReturnValue(true);
      cancelCodeViewerEditMode();

      await vi.advanceTimersByTimeAsync(400);

      expect(
        invokeMock.mock.calls.some(
          ([command]) => command === "lint_papyrus_script",
        ),
      ).toBe(false);
      vi.useRealTimers();
    });

    it("lintPapyrusScript returns no findings and logs a failed live lint instead of throwing", async () => {
      invokeMock.mockRejectedValue(new Error("cli unavailable"));
      vi.spyOn(console, "error").mockImplementation(() => {});

      const findings = await lintPapyrusScript("ScriptName Test\n");

      expect(findings).toEqual([]);
      expect(console.error).toHaveBeenCalledWith(expect.any(Error));
    });
  });
});

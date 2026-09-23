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
import { type Diagnostic } from "./backend-types";
import { openCodeViewer } from "./code-viewer-dialog";
import { enterCodeViewerEditMode } from "./live-edit-persist";

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

  describe("edit highlighting", () => {
    it("keeps linter severities visible in the editor and describes every finding in the accessible label", async () => {
      await openWithSource("line one\nline two\n", [
        { line: 2, column: 3, message: "[warning] risky edit" },
        { line: 2, column: 5, message: "[error] broken edit" },
      ]);

      enterCodeViewerEditMode();

      const lines = highlightCode().querySelectorAll(
        ".code-viewer__editor-line",
      );
      expect(lines).toHaveLength(3);
      expect(lines[1].classList.contains("code-viewer__line--error")).toBe(
        true,
      );
      // The highlight layer's own per-line title would never actually be
      // hoverable (the textarea on top of it intercepts every pointer
      // event), so it carries no title of its own - see the
      // "hovered line" tooltip test below for how the textarea's title is
      // kept in sync instead.
      expect(lines[1].hasAttribute("title")).toBe(false);
      expect(textarea().getAttribute("aria-label")).toContain(
        "Line 2, column 3: [warning] risky edit",
      );
      expect(textarea().getAttribute("aria-label")).toContain(
        "Line 2, column 5: [error] broken edit",
      );
    });

    it("renders one line number per source line in the gutter, kept in sync as the user types", async () => {
      await openWithSource("line one\nline two\n");
      enterCodeViewerEditMode();

      const gutterLines = () =>
        Array.from(
          document.querySelectorAll(
            "#code-viewer-editor-gutter .code-viewer__editor-gutter-line",
          ),
        ).map((el) => el.textContent);
      expect(gutterLines()).toEqual(["1", "2", "3"]);

      textarea().value = "line one\nline two\nline three\n";
      textarea().dispatchEvent(new Event("input"));

      expect(gutterLines()).toEqual(["1", "2", "3", "4"]);
    });

    it("renders each blank source line as its own line element with no stray whitespace between line elements", async () => {
      // Regression test: the highlight overlay's line spans are `display:
      // block` (styles.css), so a literal "\n" joining them used to render,
      // under `white-space: pre`, as an extra blank line stacked on top of
      // each blank source line's own (empty, so zero-height) span - visibly
      // desyncing the overlay's blank lines from the textarea's underneath
      // it. The overlay must instead emit exactly one line element per
      // source line, back to back, with nothing textual between them.
      await openWithSource("ScriptName Foo\n\nFunction Bar()\nEndFunction\n");

      enterCodeViewerEditMode();

      const lines = Array.from(
        highlightCode().querySelectorAll(".code-viewer__editor-line"),
      );
      expect(lines).toHaveLength(5);
      expect(lines.map((line) => line.textContent)).toEqual([
        "ScriptName Foo",
        "",
        "Function Bar()",
        "EndFunction",
        "",
      ]);
      for (let i = 0; i < lines.length - 1; i++) {
        expect(lines[i].nextSibling).toBe(lines[i + 1]);
      }
    });
  });
});

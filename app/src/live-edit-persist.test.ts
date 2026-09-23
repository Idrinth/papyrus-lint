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

import { confirmDetectedConfig, invokeImplFor } from "./test/harness";
import { type Diagnostic } from "./backend-types";
import { handleDroppedPaths } from "./drop";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { openCodeViewer } from "./code-viewer-dialog";
import {
  cancelCodeViewerEditMode,
  enterCodeViewerEditMode,
  isCodeViewerEditDirty,
  saveAndCompileCodeViewerEdits,
  saveCodeViewerEdits,
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

  function panelHidden(id: string) {
    return document.querySelector<HTMLElement>(id)!.hidden;
  }

  describe("enterCodeViewerEditMode", () => {
    it("loads the source into the textarea, highlights it, and switches to edit mode", async () => {
      await openWithSource('Debug.Trace("hi")\n');

      enterCodeViewerEditMode();

      expect(textarea().value).toBe('Debug.Trace("hi")\n');
      expect(highlightCode().innerHTML).toContain("Debug");
      expect(panelHidden("#code-viewer-view")).toBe(true);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
      expect(panelHidden("#code-viewer-edit")).toBe(true);
      expect(panelHidden("#code-viewer-save")).toBe(false);
      expect(panelHidden("#code-viewer-cancel")).toBe(false);
    });

    it("does nothing when the code viewer hasn't finished loading", async () => {
      // A failed read leaves codeViewerState null (openCodeViewer resets it
      // to null up front and only repopulates it after a successful read).
      invokeMock.mockRejectedValue(new Error("permission denied"));
      await openCodeViewer("/a.psc", []);

      enterCodeViewerEditMode();

      expect(panelHidden("#code-viewer-editor")).toBe(true);
    });
  });

  describe("isCodeViewerEditDirty", () => {
    it("is false right after entering edit mode and true once the textarea changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();

      expect(isCodeViewerEditDirty()).toBe(false);

      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));

      expect(isCodeViewerEditDirty()).toBe(true);
      // The "input" listener re-highlights the edited text as it changes.
      expect(highlightCode().innerHTML).toContain("2");
    });

    it("is false in view mode even with a stale textarea value", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(true);
      cancelCodeViewerEditMode();

      expect(isCodeViewerEditDirty()).toBe(false);
    });

    it("is false right after entering edit mode on a CRLF-saved file", async () => {
      // A textarea's value getter normalizes CRLF to LF even though nothing
      // was typed, so comparing it against the CRLF source verbatim would
      // read as dirty with no edit having happened.
      await openWithSource("Int x = 1\r\nInt y = 2\r\n");
      enterCodeViewerEditMode();

      expect(isCodeViewerEditDirty()).toBe(false);
    });
  });

  describe("cancelCodeViewerEditMode", () => {
    it("returns to view mode without confirming when there are no unsaved changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const confirmSpy = vi.spyOn(window, "confirm");

      cancelCodeViewerEditMode();

      expect(confirmSpy).not.toHaveBeenCalled();
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("returns to view mode without confirming on a CRLF-saved file with no unsaved changes", async () => {
      await openWithSource("Int x = 1\r\nInt y = 2\r\n");
      enterCodeViewerEditMode();
      const confirmSpy = vi.spyOn(window, "confirm");

      cancelCodeViewerEditMode();

      expect(confirmSpy).not.toHaveBeenCalled();
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("stays in edit mode when the user declines to discard unsaved changes", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(false);

      cancelCodeViewerEditMode();

      expect(panelHidden("#code-viewer-editor")).toBe(false);
    });

    it("discards unsaved changes and returns to view mode when the user confirms", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      vi.spyOn(window, "confirm").mockReturnValue(true);

      cancelCodeViewerEditMode();

      expect(panelHidden("#code-viewer-view")).toBe(false);
    });
  });

  describe("saveCodeViewerEdits", () => {
    it("writes the file, re-lints it, and returns to view mode", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [
          { line: 1, column: 1, message: "[warning] changed" },
        ],
      });

      await saveCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", {
        path: "/a.psc",
        contents: "Int x = 2\n",
      });
      expect(panelHidden("#code-viewer-view")).toBe(false);
      expect(isCodeViewerEditDirty()).toBe(false);
    });

    it("updates the matching lint results entry when one is open", async () => {
      // activeSeverities is module state that outlives mountFixture(), so an
      // earlier test unchecking a severity filter would otherwise leak in.
      const errorFilter =
        document.querySelector<HTMLInputElement>("#filter-error")!;
      errorFilter.checked = true;
      errorFilter.dispatchEvent(new Event("change"));

      invokeImplFor({
        parse_achlist_file: () => ["A.psc"],
        load_lint_config: () => DEFAULT_LINT_CONFIG,
        load_compiler_path: () => null,
        load_compile_check: () => false,
        load_script_roots: () => [],
        parse_psc_file: () => ({ name: "A" }),
        lint_psc_file: () => [],
      });
      const pending = handleDroppedPaths(["/proj/list.achlist"]);
      await confirmDetectedConfig();
      await pending;
      // The dropped file's outcome is keyed by the same path handed to
      // parse_psc_file above, so the code viewer must be opened on it too.
      invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
      await openCodeViewer("A.psc", []);
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [
          { line: 1, column: 1, message: "[error] changed" },
        ],
      });

      await saveCodeViewerEdits();

      expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(
        1,
      );
    });

    it("shows a failure and stays in edit mode when writing fails", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeMock.mockRejectedValue(new Error("disk full"));
      vi.spyOn(console, "error").mockImplementation(() => {});
      const saveButton =
        document.querySelector<HTMLButtonElement>("#code-viewer-save")!;

      await saveCodeViewerEdits();

      expect(saveButton.textContent).toBe("Save failed");
      expect(saveButton.disabled).toBe(false);
      expect(panelHidden("#code-viewer-editor")).toBe(false);

      vi.advanceTimersByTime(2000);
      expect(saveButton.textContent).toBe("Save");
      vi.useRealTimers();
    });
  });

  describe("saveAndCompileCodeViewerEdits", () => {
    function compileOutputEl() {
      return document.querySelector<HTMLElement>(
        "#code-viewer-compile-output",
      )!;
    }

    it("saves the file and shows the compiler's output", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({
          success: true,
          stdout: "Compilation succeeded.\n",
          stderr: "",
        }),
      });

      await saveAndCompileCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", {
        path: "/a.psc",
        contents: "Int x = 2\n",
      });
      expect(invokeMock).toHaveBeenCalledWith(
        "compile_psc_file",
        expect.objectContaining({ path: "/a.psc", game: "skyrim" }),
      );
      expect(compileOutputEl().hidden).toBe(false);
      expect(compileOutputEl().textContent).toContain("Compilation succeeded.");
      expect(
        compileOutputEl().classList.contains("psc-result__compile-output--ok"),
      ).toBe(true);
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("marks a compiler-reported failure", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({
          success: false,
          stdout: "",
          stderr: "Broken.psc(3,1): error\n",
        }),
      });
      const button = document.querySelector<HTMLButtonElement>(
        "#code-viewer-save-compile",
      )!;

      await saveAndCompileCodeViewerEdits();

      expect(compileOutputEl().textContent).toContain("Broken.psc(3,1): error");
      expect(
        compileOutputEl().classList.contains(
          "psc-result__compile-output--error",
        ),
      ).toBe(true);
      expect(button.disabled).toBe(false);
      expect(button.textContent).toBe("Save & Compile");
    });

    it("does not compile, and reports a failure, when saving fails", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeMock.mockRejectedValue(new Error("disk full"));
      vi.spyOn(console, "error").mockImplementation(() => {});
      const button = document.querySelector<HTMLButtonElement>(
        "#code-viewer-save-compile",
      )!;

      await saveAndCompileCodeViewerEdits();

      expect(button.textContent).toBe("Save failed");
      expect(button.disabled).toBe(false);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
      expect(invokeMock).not.toHaveBeenCalledWith(
        "compile_psc_file",
        expect.anything(),
      );
      expect(compileOutputEl().hidden).toBe(true);

      vi.advanceTimersByTime(2000);
      expect(button.textContent).toBe("Save & Compile");
      vi.useRealTimers();
    });

    it("clears a previous compile result when editing starts again", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: true, stdout: "ok", stderr: "" }),
      });
      await saveAndCompileCodeViewerEdits();
      expect(compileOutputEl().hidden).toBe(false);

      enterCodeViewerEditMode();

      expect(compileOutputEl().hidden).toBe(true);
      expect(compileOutputEl().textContent).toBe("");
    });
  });
});

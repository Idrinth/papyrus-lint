import { describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import { confirmDetectedConfig, invokeImplFor } from "./test/harness";
import { handleDroppedPaths, lintPapyrusScript, listScriptMembers, type Diagnostic } from "./main";
import { DEFAULT_LINT_CONFIG } from "./config";
import { openCodeViewer } from "./code-viewer";
import {
  applyAutocompleteSelection,
  cancelCodeViewerEditMode,
  enterCodeViewerEditMode,
  handleAutocompleteKeydown,
  handleEditorTabKeydown,
  hideAutocomplete,
  isCodeViewerEditDirty,
  saveAndCompileCodeViewerEdits,
  saveCodeViewerEdits,
  updateAutocomplete,
} from "./live-edit";

describe("code viewer edit mode", () => {
  async function openWithSource(source: string, findings: Diagnostic[] = []) {
    invokeImplFor({ read_psc_file: () => source });
    await openCodeViewer("/a.psc", findings);
  }

  function textarea() {
    return document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
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

    it("keeps linter severities visible in the editor and describes every finding in the accessible label", async () => {
      await openWithSource("line one\nline two\n", [
        { line: 2, column: 3, message: "[warning] risky edit" },
        { line: 2, column: 5, message: "[error] broken edit" },
      ]);

      enterCodeViewerEditMode();

      const lines = highlightCode().querySelectorAll(".code-viewer__editor-line");
      expect(lines).toHaveLength(3);
      expect(lines[1].classList.contains("code-viewer__line--error")).toBe(true);
      // The highlight layer's own per-line title would never actually be
      // hoverable (the textarea on top of it intercepts every pointer
      // event), so it carries no title of its own - see the
      // "hovered line" tooltip test below for how the textarea's title is
      // kept in sync instead.
      expect(lines[1].hasAttribute("title")).toBe(false);
      expect(textarea().getAttribute("aria-label")).toContain("Line 2, column 3: [warning] risky edit");
      expect(textarea().getAttribute("aria-label")).toContain("Line 2, column 5: [error] broken edit");
    });

    it("renders one line number per source line in the gutter, kept in sync as the user types", async () => {
      await openWithSource("line one\nline two\n");
      enterCodeViewerEditMode();

      const gutterLines = () =>
        Array.from(document.querySelectorAll("#code-viewer-editor-gutter .code-viewer__editor-gutter-line")).map(
          (el) => el.textContent,
        );
      expect(gutterLines()).toEqual(["1", "2", "3"]);

      textarea().value = "line one\nline two\nline three\n";
      textarea().dispatchEvent(new Event("input"));

      expect(gutterLines()).toEqual(["1", "2", "3", "4"]);
    });

    it("updates the textarea's tooltip to only the hovered line's findings, not the whole file's", async () => {
      await openWithSource("line one\nline two\nline three\n", [
        { line: 1, column: 1, message: "[info] first line" },
        { line: 3, column: 1, message: "[warning] third line" },
      ]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");
      expect(ta.title).not.toContain("[warning] third line");

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 55 }));
      expect(ta.title).toContain("[warning] third line");
      expect(ta.title).not.toContain("[info] first line");

      ta.dispatchEvent(new MouseEvent("mouseleave"));
      expect(ta.title).toBe("");
    });

    it("re-evaluates the tooltip on scroll, so a stationary pointer over a newly scrolled-in line isn't left describing the old one", async () => {
      await openWithSource("line one\nline two\nline three\n", [
        { line: 1, column: 1, message: "[info] first line" },
        { line: 3, column: 1, message: "[warning] third line" },
      ]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      const computedStyle = vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      // Hover line 1 while unscrolled.
      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");

      // Scroll line 3 underneath that same, still-stationary pointer
      // position (scrollTop of 40px shifts offsetY from 0 to 40, i.e. line
      // 3 under a mock that reports no scroll of its own).
      Object.defineProperty(ta, "scrollTop", { value: 40, configurable: true });
      ta.dispatchEvent(new Event("scroll"));

      expect(ta.title).toContain("[warning] third line");
      expect(ta.title).not.toContain("[info] first line");

      computedStyle.mockRestore();
    });

    it("clears the tooltip on scroll once the mouse has already left the editor, instead of reusing a stale position", async () => {
      await openWithSource("line one\nline two\nline three\n", [{ line: 1, column: 1, message: "[info] first line" }]);
      enterCodeViewerEditMode();
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 100,
        right: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
      } as CSSStyleDeclaration);

      ta.dispatchEvent(new MouseEvent("mousemove", { clientY: 10 }));
      expect(ta.title).toContain("[info] first line");

      ta.dispatchEvent(new MouseEvent("mouseleave"));
      ta.dispatchEvent(new Event("scroll"));

      expect(ta.title).toBe("");
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

      const lines = Array.from(highlightCode().querySelectorAll(".code-viewer__editor-line"));
      expect(lines).toHaveLength(5);
      expect(lines.map((line) => line.textContent)).toEqual(["ScriptName Foo", "", "Function Bar()", "EndFunction", ""]);
      for (let i = 0; i < lines.length - 1; i++) {
        expect(lines[i].nextSibling).toBe(lines[i + 1]);
      }
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

  describe("live linting while editing (scheduleLiveEditLint)", () => {
    it("lints the textarea's current contents via lint_papyrus_script after debouncing an edit", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      invokeImplFor({
        lint_papyrus_script: () => [{ line: 1, column: 1, message: "[error] broken edit" }],
      });

      textarea().value = "Int x = 2\n";
      textarea().dispatchEvent(new Event("input"));
      await vi.advanceTimersByTimeAsync(400);

      expect(invokeMock).toHaveBeenCalledWith(
        "lint_papyrus_script",
        expect.objectContaining({ source: "Int x = 2\n" }),
      );
      expect(highlightCode().querySelectorAll(".code-viewer__line--error")).toHaveLength(1);
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

      const liveLintCalls = invokeMock.mock.calls.filter(([command]) => command === "lint_papyrus_script");
      expect(liveLintCalls).toHaveLength(1);
      expect(liveLintCalls[0][1]).toMatchObject({ source: "Int x = 123\n" });
      vi.useRealTimers();
    });

    it("keeps the last saved findings visible until the first live lint resolves", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n", [{ line: 1, column: 1, message: "[warning] stale" }]);
      enterCodeViewerEditMode();

      expect(highlightCode().querySelectorAll(".code-viewer__line--warning")).toHaveLength(1);
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

      expect(document.querySelectorAll("#code-viewer-editor-highlight .code-viewer__line--error")).toHaveLength(0);
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

      expect(invokeMock.mock.calls.some(([command]) => command === "lint_papyrus_script")).toBe(false);
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
        lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] changed" }],
      });

      await saveCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", { path: "/a.psc", contents: "Int x = 2\n" });
      expect(panelHidden("#code-viewer-view")).toBe(false);
      expect(isCodeViewerEditDirty()).toBe(false);
    });

    it("updates the matching lint results entry when one is open", async () => {
      // activeSeverities is module state that outlives mountFixture(), so an
      // earlier test unchecking a severity filter would otherwise leak in.
      const errorFilter = document.querySelector<HTMLInputElement>("#filter-error")!;
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
        lint_psc_file: () => [{ line: 1, column: 1, message: "[error] changed" }],
      });

      await saveCodeViewerEdits();

      expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    });

    it("shows a failure and stays in edit mode when writing fails", async () => {
      vi.useFakeTimers();
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeMock.mockRejectedValue(new Error("disk full"));
      vi.spyOn(console, "error").mockImplementation(() => {});
      const saveButton = document.querySelector<HTMLButtonElement>("#code-viewer-save")!;

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
      return document.querySelector<HTMLElement>("#code-viewer-compile-output")!;
    }

    it("saves the file and shows the compiler's output", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";

      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.\n", stderr: "" }),
      });

      await saveAndCompileCodeViewerEdits();

      expect(invokeMock).toHaveBeenCalledWith("write_psc_file", { path: "/a.psc", contents: "Int x = 2\n" });
      expect(invokeMock).toHaveBeenCalledWith("compile_psc_file", expect.objectContaining({ path: "/a.psc" }));
      expect(compileOutputEl().hidden).toBe(false);
      expect(compileOutputEl().textContent).toContain("Compilation succeeded.");
      expect(compileOutputEl().classList.contains("psc-result__compile-output--ok")).toBe(true);
      expect(panelHidden("#code-viewer-view")).toBe(false);
    });

    it("marks a compiler-reported failure", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      textarea().value = "Int x = 2\n";
      invokeImplFor({
        write_psc_file: () => undefined,
        lint_psc_file: () => [],
        compile_psc_file: () => ({ success: false, stdout: "", stderr: "Broken.psc(3,1): error\n" }),
      });
      const button = document.querySelector<HTMLButtonElement>("#code-viewer-save-compile")!;

      await saveAndCompileCodeViewerEdits();

      expect(compileOutputEl().textContent).toContain("Broken.psc(3,1): error");
      expect(compileOutputEl().classList.contains("psc-result__compile-output--error")).toBe(true);
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
      const button = document.querySelector<HTMLButtonElement>("#code-viewer-save-compile")!;

      await saveAndCompileCodeViewerEdits();

      expect(button.textContent).toBe("Save failed");
      expect(button.disabled).toBe(false);
      expect(panelHidden("#code-viewer-editor")).toBe(false);
      expect(invokeMock).not.toHaveBeenCalledWith("compile_psc_file", expect.anything());
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

  describe("autocompletion", () => {
    const SELF_MEMBER_SCRIPT = "ScriptName Example\n\nFunction Run()\n    self.\nEndFunction\n";

    function autocompleteEl() {
      return document.querySelector<HTMLElement>("#code-viewer-autocomplete")!;
    }

    // Enters edit mode on SELF_MEMBER_SCRIPT and places the caret right
    // after "self.", the spot autocompletion should trigger from.
    async function openWithCursorAfterSelfDot() {
      await openWithSource(SELF_MEMBER_SCRIPT);
      enterCodeViewerEditMode();
      const field = textarea();
      const cursor = field.value.indexOf("self.") + "self.".length;
      field.setSelectionRange(cursor, cursor);
      return field;
    }

    it("does nothing while not in edit mode", async () => {
      await openWithSource(SELF_MEMBER_SCRIPT);

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
    });

    it("queries list_script_members for the receiver's declared type and shows the results", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: { name: "String", is_array: false },
            is_global: false,
            is_native: true,
            is_event: false,
          },
          { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false } },
        ],
      });

      await updateAutocomplete();

      expect(invokeMock).toHaveBeenCalledWith("list_script_members", expect.objectContaining({ typeName: "Example" }));
      expect(autocompleteEl().hidden).toBe(false);
      expect(autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item")).toHaveLength(2);
    });

    it("hides the dropdown when the cursor isn't right after a member access", async () => {
      await openWithSource("ScriptName Example\n\nFunction Run()\n    Int i = 0\nEndFunction\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(field.value.length, field.value.length);

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
    });

    it("hides the dropdown when text is selected", async () => {
      const field = await openWithCursorAfterSelfDot();
      field.setSelectionRange(0, 4);

      await updateAutocomplete();

      expect(invokeMock).not.toHaveBeenCalledWith("list_script_members", expect.anything());
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("hides and clears the dropdown when the backend has no matching members", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({ list_script_members: () => [] });

      await updateAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().children).toHaveLength(0);
    });

    it("applyAutocompleteSelection splices the chosen member's insertion text in place of the typed prefix", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: null,
            is_global: false,
            is_native: false,
            is_event: false,
          },
        ],
      });
      await updateAutocomplete();

      applyAutocompleteSelection(0);

      expect(field.value).toBe("ScriptName Example\n\nFunction Run()\n    self.GetName(\nEndFunction\n");
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("navigates with the arrow keys and accepts the highlighted entry on Enter", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } },
          { kind: "property", name: "BProp", type_name: { name: "Int", is_array: false } },
        ],
      });
      await updateAutocomplete();

      const downEvent = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
      handleAutocompleteKeydown(downEvent);
      expect(downEvent.defaultPrevented).toBe(true);
      expect(autocompleteEl().querySelector(".code-viewer__autocomplete-item--active")?.textContent).toContain(
        "BProp",
      );

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Enter", cancelable: true }));

      expect(field.value).toContain("self.BProp");
    });

    it("wraps upward, accepts with Tab, and ignores unrelated keys", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } },
          { kind: "property", name: "BProp", type_name: { name: "Int", is_array: false } },
        ],
      });
      await updateAutocomplete();

      const unrelated = new KeyboardEvent("keydown", { key: "Shift", cancelable: true });
      handleAutocompleteKeydown(unrelated);
      expect(unrelated.defaultPrevented).toBe(false);

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "ArrowUp", cancelable: true }));
      expect(autocompleteEl().querySelector(".code-viewer__autocomplete-item--active")?.textContent).toContain(
        "BProp",
      );
      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));
      expect(field.value).toContain("self.BProp");
    });

    it("leaves Tab to the dropdown instead of also inserting a literal tab", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      const event = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
      handleAutocompleteKeydown(event);
      handleEditorTabKeydown(event);

      expect(field.value).toContain("self.AProp");
      expect(field.value).not.toContain("\t");
    });

    it("accepts a completion when its dropdown item is clicked", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          { kind: "property", name: "Target", type_name: { name: "ObjectReference", is_array: false } },
        ],
      });
      await updateAutocomplete();

      autocompleteEl()
        .querySelector<HTMLButtonElement>(".code-viewer__autocomplete-item")!
        .dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));

      expect(field.value).toContain("self.Target");
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("ignores selection and navigation requests when no completion is available", () => {
      applyAutocompleteSelection(99);
      const event = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
      handleAutocompleteKeydown(event);
      expect(event.defaultPrevented).toBe(false);
    });

    it("dismisses the dropdown on Escape without touching the textarea", async () => {
      const field = await openWithCursorAfterSelfDot();
      const beforeEscape = field.value;
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));

      expect(autocompleteEl().hidden).toBe(true);
      expect(field.value).toBe(beforeEscape);
    });

    it("hideAutocomplete clears any pending dropdown", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }],
      });
      await updateAutocomplete();

      hideAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item")).toHaveLength(0);
    });

    it("does not reopen the dropdown when a hidden lookup finishes", async () => {
      await openWithCursorAfterSelfDot();
      let finishLookup!: (members: unknown[]) => void;
      invokeImplFor({
        list_script_members: () =>
          new Promise<unknown[]>((resolve) => {
            finishLookup = resolve;
          }),
      });
      const pendingUpdate = updateAutocomplete();

      hideAutocomplete();
      finishLookup([{ kind: "property", name: "AProp", type_name: { name: "Int", is_array: false } }]);
      await pendingUpdate;

      expect(autocompleteEl().hidden).toBe(true);
      expect(autocompleteEl().children).toHaveLength(0);
    });

    it("keeps the newest results when an older lookup finishes last", async () => {
      const field = await openWithCursorAfterSelfDot();
      const finishes: Array<(members: unknown[]) => void> = [];
      invokeImplFor({
        list_script_members: () =>
          new Promise<unknown[]>((resolve) => {
            finishes.push(resolve);
          }),
      });
      const olderUpdate = updateAutocomplete();
      field.setRangeText("b", field.selectionStart, field.selectionEnd, "end");
      const newerUpdate = updateAutocomplete();

      finishes[1]([{ kind: "property", name: "Better", type_name: { name: "Int", is_array: false } }]);
      await newerUpdate;
      finishes[0]([{ kind: "property", name: "Ancient", type_name: { name: "Int", is_array: false } }]);
      await olderUpdate;

      expect(autocompleteEl().textContent).toContain("Better");
      expect(autocompleteEl().textContent).not.toContain("Ancient");
    });

    it("listScriptMembers logs and returns an empty list when the backend call fails", async () => {
      invokeMock.mockRejectedValue(new Error("lookup failed"));
      vi.spyOn(console, "error").mockImplementation(() => {});

      await expect(listScriptMembers("Example")).resolves.toEqual([]);
    });

    it("shows the active member's documentation comment as help under its label", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: { name: "String", is_array: false },
            is_global: false,
            is_native: true,
            is_event: false,
            doc: "The display name of this form",
          },
          { kind: "property", name: "TargetRef", type_name: { name: "ObjectReference", is_array: false }, doc: "Where we go" },
        ],
      });

      await updateAutocomplete();

      const active = autocompleteEl().querySelector(".code-viewer__autocomplete-item--active");
      expect(active?.querySelector(".code-viewer__autocomplete-item-doc")?.textContent).toBe("The display name of this form");
      expect(autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item-doc")).toHaveLength(1);

      handleAutocompleteKeydown(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }));

      const nextActive = autocompleteEl().querySelector(".code-viewer__autocomplete-item--active");
      expect(nextActive?.querySelector(".code-viewer__autocomplete-item-doc")?.textContent).toBe("Where we go");
    });

    it("overlays unsaved documentation comments from the buffer onto self members", async () => {
      const source = "ScriptName Example\n{script}\n\nFunction GetName()\n{Fresh local help}\nEndFunction\n\nFunction Run()\n    self.\nEndFunction\n";
      await openWithSource(source);
      enterCodeViewerEditMode();
      const field = textarea();
      const cursor = field.value.indexOf("self.") + "self.".length;
      field.setSelectionRange(cursor, cursor);
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "function",
            name: "GetName",
            params: [],
            return_type: null,
            is_global: false,
            is_native: false,
            is_event: false,
            doc: "stale disk copy",
          },
        ],
      });

      await updateAutocomplete();

      expect(autocompleteEl().querySelector(".code-viewer__autocomplete-item-doc")?.textContent).toBe("Fresh local help");
    });
  });

  describe("documentation hover", () => {
    function hoverLine(line: number) {
      const ta = textarea();
      vi.spyOn(ta, "getBoundingClientRect").mockReturnValue({
        top: 0,
        left: 0,
        bottom: 200,
        right: 200,
        width: 200,
        height: 200,
        x: 0,
        y: 0,
        toJSON() {},
      } as DOMRect);
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "10px",
        paddingLeft: "0px",
        fontSize: "",
        fontFamily: "",
        fontWeight: "",
        letterSpacing: "",
      } as CSSStyleDeclaration);
      const clientY = 10 + (line - 1) * 20;
      ta.dispatchEvent(new MouseEvent("mousemove", { clientX: 0, clientY }));
    }

    it("shows a declaration's documentation comment when hovering its header line", async () => {
      await openWithSource("ScriptName Example\n{A documented script}\n\nFunction DoThing()\n{Does the thing}\nEndFunction\n");
      enterCodeViewerEditMode();

      hoverLine(1);
      expect(textarea().title).toBe("A documented script");

      hoverLine(4);
      expect(textarea().title).toBe("Does the thing");
    });

    it("keeps lint findings in the tooltip under the documentation comment", async () => {
      await openWithSource("ScriptName Example\n{A documented script}\n\nFunction DoThing()\nEndFunction\n", [
        { line: 1, column: 1, message: "[info] first line" },
      ]);
      enterCodeViewerEditMode();

      hoverLine(1);
      expect(textarea().title).toContain("A documented script");
      expect(textarea().title).toContain("[info] first line");
    });
  });

  describe("handleEditorTabKeydown", () => {
    it("inserts a literal tab at the caret instead of letting focus leave the textarea", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
      handleEditorTabKeydown(event);

      expect(event.defaultPrevented).toBe(true);
      expect(field.value).toBe("Int\t x = 1\n");
      expect(field.selectionStart).toBe(4);
    });

    it("replaces the current selection with a tab", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(0, 3);

      handleEditorTabKeydown(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));

      expect(field.value).toBe("\t x = 1\n");
    });

    it("ignores keys other than Tab", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
      handleEditorTabKeydown(event);

      expect(event.defaultPrevented).toBe(false);
      expect(field.value).toBe("Int x = 1\n");
    });
  });
});

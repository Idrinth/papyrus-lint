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

  describe("finding tooltip", () => {
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
      const computedStyle = vi
        .spyOn(window, "getComputedStyle")
        .mockReturnValue({
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
      await openWithSource("line one\nline two\nline three\n", [
        { line: 1, column: 1, message: "[info] first line" },
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

      ta.dispatchEvent(new MouseEvent("mouseleave"));
      ta.dispatchEvent(new Event("scroll"));

      expect(ta.title).toBe("");
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

    function enableCharacterHitTesting() {
      vi.spyOn(
        HTMLElement.prototype,
        "getBoundingClientRect",
      ).mockImplementation(function (this: HTMLElement) {
        const width = this instanceof HTMLSpanElement ? 100 : 200;
        return {
          top: 0,
          left: 0,
          bottom: 200,
          right: width,
          width,
          height: 200,
          x: 0,
          y: 0,
          toJSON() {},
        } as DOMRect;
      });
      vi.spyOn(window, "getComputedStyle").mockReturnValue({
        lineHeight: "20px",
        paddingTop: "0px",
        paddingLeft: "0px",
        fontSize: "16px",
        fontFamily: "monospace",
        fontWeight: "400",
        letterSpacing: "0px",
      } as CSSStyleDeclaration);
    }

    it("shows a declaration's documentation comment when hovering its header line", async () => {
      await openWithSource(
        "ScriptName Example\n{A documented script}\n\nFunction DoThing()\n{Does the thing}\nEndFunction\n",
      );
      enterCodeViewerEditMode();

      hoverLine(1);
      expect(textarea().title).toBe("A documented script");

      hoverLine(4);
      expect(textarea().title).toBe("Does the thing");
    });

    it("keeps lint findings in the tooltip under the documentation comment", async () => {
      await openWithSource(
        "ScriptName Example\n{A documented script}\n\nFunction DoThing()\nEndFunction\n",
        [{ line: 1, column: 1, message: "[info] first line" }],
      );
      enterCodeViewerEditMode();

      hoverLine(1);
      expect(textarea().title).toContain("A documented script");
      expect(textarea().title).toContain("[info] first line");
    });

    it("loads documentation for a member under the pointer and reuses the cached type members", async () => {
      const source =
        "ScriptName Example\nObjectReference Property Target Auto\nFunction Run()\nTarget.GetName()\nEndFunction\n";
      await openWithSource(source, [
        { line: 4, column: 1, message: "[warning] check this call" },
      ]);
      enterCodeViewerEditMode();
      enableCharacterHitTesting();
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
            doc: "Returns this object's display name",
          },
        ],
      });

      const ta = textarea();
      const hoverMember = () =>
        ta.dispatchEvent(
          new MouseEvent("mousemove", { clientX: 80, clientY: 65 }),
        );
      hoverMember();
      await vi.waitFor(() =>
        expect(ta.title).toContain("Returns this object's display name"),
      );
      expect(ta.title).toContain("[warning] check this call");

      hoverMember();
      await vi.waitFor(() => {
        const lookups = invokeMock.mock.calls.filter(
          ([command]) => command === "list_script_members",
        );
        expect(lookups).toHaveLength(1);
      });
    });

    it("does not request member documentation when the receiver's type is unknown", async () => {
      await openWithSource(
        "ScriptName Example\nFunction Run()\nmissing.GetName()\nEndFunction\n",
      );
      enterCodeViewerEditMode();
      enableCharacterHitTesting();

      textarea().dispatchEvent(
        new MouseEvent("mousemove", { clientX: 90, clientY: 45 }),
      );
      await Promise.resolve();

      expect(
        invokeMock.mock.calls.some(
          ([command]) => command === "list_script_members",
        ),
      ).toBe(false);
      expect(textarea().title).toBe("");
    });

    it("ignores member documentation that resolves after the pointer leaves the editor", async () => {
      await openWithSource(
        "ScriptName Example\nObjectReference Property Target Auto\nFunction Run()\nTarget.GetName()\nEndFunction\n",
      );
      enterCodeViewerEditMode();
      enableCharacterHitTesting();
      let finishLookup!: (members: unknown[]) => void;
      invokeImplFor({
        list_script_members: () =>
          new Promise<unknown[]>((resolve) => {
            finishLookup = resolve;
          }),
      });

      const ta = textarea();
      ta.dispatchEvent(
        new MouseEvent("mousemove", { clientX: 80, clientY: 65 }),
      );
      ta.dispatchEvent(new MouseEvent("mouseleave"));
      finishLookup([
        {
          kind: "function",
          name: "GetName",
          params: [],
          return_type: null,
          is_global: false,
          is_native: false,
          is_event: false,
          doc: "Too late",
        },
      ]);
      await Promise.resolve();
      await Promise.resolve();

      expect(ta.title).toBe("");
    });
  });
});

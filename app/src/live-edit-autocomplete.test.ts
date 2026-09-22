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
import { listScriptMembers, type Diagnostic } from "./backend";
import { openCodeViewer } from "./code-viewer-dialog";
import {
  applyAutocompleteSelection,
  handleAutocompleteKeydown,
  handleEditorTabKeydown,
  hideAutocomplete,
  updateAutocomplete,
} from "./live-edit-autocomplete";
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

  describe("autocompletion", () => {
    const SELF_MEMBER_SCRIPT =
      "ScriptName Example\n\nFunction Run()\n    self.\nEndFunction\n";

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
          {
            kind: "property",
            name: "TargetRef",
            type_name: { name: "ObjectReference", is_array: false },
          },
        ],
      });

      await updateAutocomplete();

      expect(invokeMock).toHaveBeenCalledWith(
        "list_script_members",
        expect.objectContaining({ typeName: "Example" }),
      );
      expect(autocompleteEl().hidden).toBe(false);
      expect(
        autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item"),
      ).toHaveLength(2);
    });

    it("hides the dropdown when the cursor isn't right after a member access", async () => {
      await openWithSource(
        "ScriptName Example\n\nFunction Run()\n    Int i = 0\nEndFunction\n",
      );
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

      expect(invokeMock).not.toHaveBeenCalledWith(
        "list_script_members",
        expect.anything(),
      );
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

      expect(field.value).toBe(
        "ScriptName Example\n\nFunction Run()\n    self.GetName(\nEndFunction\n",
      );
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("navigates with the arrow keys and accepts the highlighted entry on Enter", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "AProp",
            type_name: { name: "Int", is_array: false },
          },
          {
            kind: "property",
            name: "BProp",
            type_name: { name: "Int", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      const downEvent = new KeyboardEvent("keydown", {
        key: "ArrowDown",
        cancelable: true,
      });
      handleAutocompleteKeydown(downEvent);
      expect(downEvent.defaultPrevented).toBe(true);
      expect(
        autocompleteEl().querySelector(
          ".code-viewer__autocomplete-item--active",
        )?.textContent,
      ).toContain("BProp");

      handleAutocompleteKeydown(
        new KeyboardEvent("keydown", { key: "Enter", cancelable: true }),
      );

      expect(field.value).toContain("self.BProp");
    });

    it("wraps upward, accepts with Tab, and ignores unrelated keys", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "AProp",
            type_name: { name: "Int", is_array: false },
          },
          {
            kind: "property",
            name: "BProp",
            type_name: { name: "Int", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      const unrelated = new KeyboardEvent("keydown", {
        key: "Shift",
        cancelable: true,
      });
      handleAutocompleteKeydown(unrelated);
      expect(unrelated.defaultPrevented).toBe(false);

      handleAutocompleteKeydown(
        new KeyboardEvent("keydown", { key: "ArrowUp", cancelable: true }),
      );
      expect(
        autocompleteEl().querySelector(
          ".code-viewer__autocomplete-item--active",
        )?.textContent,
      ).toContain("BProp");
      handleAutocompleteKeydown(
        new KeyboardEvent("keydown", { key: "Tab", cancelable: true }),
      );
      expect(field.value).toContain("self.BProp");
    });

    it("leaves Tab to the dropdown instead of also inserting a literal tab", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "AProp",
            type_name: { name: "Int", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      const event = new KeyboardEvent("keydown", {
        key: "Tab",
        cancelable: true,
      });
      handleAutocompleteKeydown(event);
      handleEditorTabKeydown(event);

      expect(field.value).toContain("self.AProp");
      expect(field.value).not.toContain("\t");
    });

    it("accepts a completion when its dropdown item is clicked", async () => {
      const field = await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "Target",
            type_name: { name: "ObjectReference", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      autocompleteEl()
        .querySelector<HTMLButtonElement>(".code-viewer__autocomplete-item")!
        .dispatchEvent(
          new MouseEvent("mousedown", { bubbles: true, cancelable: true }),
        );

      expect(field.value).toContain("self.Target");
      expect(autocompleteEl().hidden).toBe(true);
    });

    it("ignores selection and navigation requests when no completion is available", () => {
      applyAutocompleteSelection(99);
      const event = new KeyboardEvent("keydown", {
        key: "ArrowDown",
        cancelable: true,
      });
      handleAutocompleteKeydown(event);
      expect(event.defaultPrevented).toBe(false);
    });

    it("dismisses the dropdown on Escape without touching the textarea", async () => {
      const field = await openWithCursorAfterSelfDot();
      const beforeEscape = field.value;
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "AProp",
            type_name: { name: "Int", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      handleAutocompleteKeydown(
        new KeyboardEvent("keydown", { key: "Escape", cancelable: true }),
      );

      expect(autocompleteEl().hidden).toBe(true);
      expect(field.value).toBe(beforeEscape);
    });

    it("hideAutocomplete clears any pending dropdown", async () => {
      await openWithCursorAfterSelfDot();
      invokeImplFor({
        list_script_members: () => [
          {
            kind: "property",
            name: "AProp",
            type_name: { name: "Int", is_array: false },
          },
        ],
      });
      await updateAutocomplete();

      hideAutocomplete();

      expect(autocompleteEl().hidden).toBe(true);
      expect(
        autocompleteEl().querySelectorAll(".code-viewer__autocomplete-item"),
      ).toHaveLength(0);
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
      finishLookup([
        {
          kind: "property",
          name: "AProp",
          type_name: { name: "Int", is_array: false },
        },
      ]);
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

      finishes[1]([
        {
          kind: "property",
          name: "Better",
          type_name: { name: "Int", is_array: false },
        },
      ]);
      await newerUpdate;
      finishes[0]([
        {
          kind: "property",
          name: "Ancient",
          type_name: { name: "Int", is_array: false },
        },
      ]);
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
          {
            kind: "property",
            name: "TargetRef",
            type_name: { name: "ObjectReference", is_array: false },
            doc: "Where we go",
          },
        ],
      });

      await updateAutocomplete();

      const active = autocompleteEl().querySelector(
        ".code-viewer__autocomplete-item--active",
      );
      expect(
        active?.querySelector(".code-viewer__autocomplete-item-doc")
          ?.textContent,
      ).toBe("The display name of this form");
      expect(
        autocompleteEl().querySelectorAll(
          ".code-viewer__autocomplete-item-doc",
        ),
      ).toHaveLength(1);

      handleAutocompleteKeydown(
        new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }),
      );

      const nextActive = autocompleteEl().querySelector(
        ".code-viewer__autocomplete-item--active",
      );
      expect(
        nextActive?.querySelector(".code-viewer__autocomplete-item-doc")
          ?.textContent,
      ).toBe("Where we go");
    });

    it("overlays unsaved documentation comments from the buffer onto self members", async () => {
      const source =
        "ScriptName Example\n{script}\n\nFunction GetName()\n{Fresh local help}\nEndFunction\n\nFunction Run()\n    self.\nEndFunction\n";
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

      expect(
        autocompleteEl().querySelector(".code-viewer__autocomplete-item-doc")
          ?.textContent,
      ).toBe("Fresh local help");
    });
  });

  describe("handleEditorTabKeydown", () => {
    it("inserts a literal tab at the caret instead of letting focus leave the textarea", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", {
        key: "Tab",
        cancelable: true,
      });
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

      handleEditorTabKeydown(
        new KeyboardEvent("keydown", { key: "Tab", cancelable: true }),
      );

      expect(field.value).toBe("\t x = 1\n");
    });

    it("ignores keys other than Tab", async () => {
      await openWithSource("Int x = 1\n");
      enterCodeViewerEditMode();
      const field = textarea();
      field.setSelectionRange(3, 3);

      const event = new KeyboardEvent("keydown", {
        key: "Enter",
        cancelable: true,
      });
      handleEditorTabKeydown(event);

      expect(event.defaultPrevented).toBe(false);
      expect(field.value).toBe("Int x = 1\n");
    });
  });
});

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
import {
  handleCodeViewerFixClick,
  handleCodeViewerFixLineClick,
  handleCodeViewerIgnoreLineClick,
  handleCodeViewerFileDisableLineClick,
  handleCodeViewerConfigDisableLineClick,
  handleCodeViewerNodiscardLineClick,
} from "./code-viewer-actions";
import { handleCodeViewerPreviewFixClick } from "./code-viewer-diff";
import { openCodeViewer, requestCloseCodeViewer, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
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

describe("handleCodeViewerFixClick", () => {
  async function openWithFixableFinding() {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
  }

  it("disables the button, repairs the file, and re-renders the viewer with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi.fn().mockResolvedValueOnce("line one  \n").mockResolvedValueOnce("line one\n"),
      repair_psc_file: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;

    const promise = handleCodeViewerFixClick();
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.objectContaining({ path: "/a.psc" }));
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
    expect(button.disabled).toBe(false);
  });

  it("hides the button once nothing is left to fix", async () => {
    await openWithFixableFinding();
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;
    expect(button.hidden).toBe(false);

    await handleCodeViewerFixClick();

    expect(button.hidden).toBe(true);
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    // A failed read leaves codeViewerState null (openCodeViewer resets it to
    // null up front and only repopulates it after a successful read).
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();

    await handleCodeViewerFixClick();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and leaves the viewer untouched when the repair fails", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_file: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = document.querySelector<HTMLButtonElement>("#code-viewer-fix")!;
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerFixClick();

    expect(button.disabled).toBe(false);
    expect(button.hidden).toBe(false);
  });
});

describe("handleCodeViewerFixLineClick", () => {
  function fixLineButton(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="fix"]')!;
  }

  it("disables the button, fixes just that line's rule, and re-renders with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi.fn().mockResolvedValueOnce("line one  \n").mockResolvedValueOnce("line one\n"),
      repair_psc_finding: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = fixLineButton();

    const promise = handleCodeViewerFixLineClick(1, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith(
      "repair_psc_finding",
      expect.objectContaining({ path: "/a.psc", rule: "trailing-whitespace", line: 1 }),
    );
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
  });

  it("fixes every distinct fixable rule found on the line", async () => {
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)  \n",
      repair_psc_finding: () => [],
    });
    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
      { line: 1, column: 5, message: "[warning] missing space after comma", rule: "comma-spacing" },
    ]);
    const button = fixLineButton();

    await handleCodeViewerFixLineClick(1, button);

    expect(invokeMock).toHaveBeenCalledWith(
      "repair_psc_finding",
      expect.objectContaining({ rule: "trailing-whitespace", line: 1 }),
    );
    expect(invokeMock).toHaveBeenCalledWith(
      "repair_psc_finding",
      expect.objectContaining({ rule: "comma-spacing", line: 1 }),
    );
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerFixLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing for a line with no fixable finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" }]);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerFixLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and keeps the finding when the per-rule fix fails", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_finding: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);
    const button = fixLineButton();
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerFixLineClick(1, button);

    expect(button.disabled).toBe(false);
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(1);
  });
});

describe("handleCodeViewerIgnoreLineClick", () => {
  function ignoreLineButton(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="ignore"]')!;
  }

  it("disables the button, adds the disable comment, and re-renders with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi
        .fn()
        .mockResolvedValueOnce("Foo(1,2)\n")
        .mockResolvedValueOnce("Foo(1,2) ; @disable comma-spacing\n"),
      add_disable_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    const button = ignoreLineButton();

    const promise = handleCodeViewerIgnoreLineClick(1, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith(
      "add_disable_comment_to_psc_line",
      expect.objectContaining({ path: "/a.psc", rules: ["comma-spacing"], line: 1 }),
    );
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
  });

  it("covers every distinct rule found on the line in one call", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      add_disable_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
      { line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
    ]);
    const button = ignoreLineButton();

    await handleCodeViewerIgnoreLineClick(1, button);

    expect(invokeMock).toHaveBeenCalledWith(
      "add_disable_comment_to_psc_line",
      expect.objectContaining({ rules: ["trailing-whitespace", "forbidden-functions"], line: 1 }),
    );
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerIgnoreLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing for a line with no rule-bearing finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] compiler error" }]);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerIgnoreLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and leaves the viewer untouched when adding the comment fails", async () => {
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      add_disable_comment_to_psc_line: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    const button = ignoreLineButton();
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerIgnoreLineClick(1, button);

    expect(button.disabled).toBe(false);
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(1);
  });
});

describe("handleCodeViewerFileDisableLineClick", () => {
  function fileDisableLineButton(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="file-disable"]')!;
  }

  it("disables the button, adds the file-disable comment, and re-renders with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi
        .fn()
        .mockResolvedValueOnce("Foo(1,2)\n")
        .mockResolvedValueOnce("Foo(1,2) ; @disable-file comma-spacing\n"),
      add_disable_file_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    const button = fileDisableLineButton();

    const promise = handleCodeViewerFileDisableLineClick(1, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith(
      "add_disable_file_comment_to_psc_line",
      expect.objectContaining({ path: "/a.psc", rules: ["comma-spacing"], line: 1 }),
    );
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
  });

  it("covers every distinct rule found on the line in one call", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      add_disable_file_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
      { line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
    ]);
    const button = fileDisableLineButton();

    await handleCodeViewerFileDisableLineClick(1, button);

    expect(invokeMock).toHaveBeenCalledWith(
      "add_disable_file_comment_to_psc_line",
      expect.objectContaining({ rules: ["trailing-whitespace", "forbidden-functions"], line: 1 }),
    );
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerFileDisableLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing for a line with no rule-bearing finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] compiler error" }]);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerFileDisableLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and leaves the viewer untouched when adding the comment fails", async () => {
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      add_disable_file_comment_to_psc_line: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    const button = fileDisableLineButton();
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerFileDisableLineClick(1, button);

    expect(button.disabled).toBe(false);
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(1);
  });
});

describe("handleCodeViewerConfigDisableLineClick", () => {
  function configDisableLineButton(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="config-disable"]')!;
  }

  it("disables the button, turns the rule off in config, and re-lints the file", async () => {
    invokeImplFor({ read_psc_file: () => "Foo(1,2)\n" });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      lint_psc_file: () => [],
    });
    const button = configDisableLineButton();
    const checkbox = document.querySelector<HTMLInputElement>("#rule-comma_spacing")!;
    expect(checkbox.checked).toBe(true);

    const promise = handleCodeViewerConfigDisableLineClick(1, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(checkbox.checked).toBe(false);
    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", expect.objectContaining({ path: "/a.psc" }));
    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);
  });

  it("turns off every configurable rule found on the line", async () => {
    invokeImplFor({ read_psc_file: () => "line one  \n" });
    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
      { line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
    ]);
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      lint_psc_file: () => [],
    });
    const button = configDisableLineButton();

    await handleCodeViewerConfigDisableLineClick(1, button);

    expect(document.querySelector<HTMLInputElement>("#rule-trailing_whitespace")!.checked).toBe(false);
    expect(document.querySelector<HTMLInputElement>("#rule-forbidden_functions")!.checked).toBe(false);
  });

  it("maps float-to-int onto the float_int_conversion config key", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1.5\n" });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] implicit float-to-int", rule: "float-to-int" }]);
    invokeImplFor({
      read_psc_file: () => "Int x = 1.5\n",
      lint_psc_file: () => [],
    });
    const button = configDisableLineButton();

    await handleCodeViewerConfigDisableLineClick(1, button);

    expect(document.querySelector<HTMLInputElement>("#rule-float_int_conversion")!.checked).toBe(false);
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerConfigDisableLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("does nothing for a line with no rule-bearing finding", async () => {
    invokeImplFor({ read_psc_file: () => 'Debug.Trace("hi")\n' });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[error] compiler error" }]);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerConfigDisableLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("handleCodeViewerNodiscardLineClick", () => {
  function nodiscardLineButton(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="nodiscard"]')!;
  }

  it("disables the button, adds the nodiscard flag, and re-renders with the re-read source", async () => {
    invokeImplFor({
      read_psc_file: vi
        .fn()
        .mockResolvedValueOnce("Int Function GetValue()\n")
        .mockResolvedValueOnce("Int Function GetValue() ; @nodiscard\n"),
      add_nodiscard_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", []);
    const button = nodiscardLineButton();

    const promise = handleCodeViewerNodiscardLineClick(1, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(invokeMock).toHaveBeenCalledWith("add_nodiscard_comment_to_psc_line", expect.objectContaining({ path: "/a.psc", line: 1 }));
    expect(document.querySelector('#code-viewer-line-1 [data-line-action="nodiscard"]')).toBeNull();
    expect(button.disabled).toBe(false);
  });

  it("does nothing when the code viewer has no loaded file", async () => {
    invokeMock.mockRejectedValue(new Error("permission denied"));
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();
    const button = document.createElement("button");

    await handleCodeViewerNodiscardLineClick(1, button);

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("re-enables the button and leaves the viewer untouched when adding the flag fails", async () => {
    invokeImplFor({
      read_psc_file: () => "Int Function GetValue()\n",
      add_nodiscard_comment_to_psc_line: () => Promise.reject(new Error("disk full")),
    });
    await openCodeViewer("/a.psc", []);
    const button = nodiscardLineButton();
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleCodeViewerNodiscardLineClick(1, button);

    expect(button.disabled).toBe(false);
    expect(document.querySelector('#code-viewer-line-1 [data-line-action="nodiscard"]')).not.toBeNull();
  });
});

describe("handleCodeViewerLineActionClick (delegated click handling)", () => {
  it("routes a click on the per-line Fix button to handleCodeViewerFixLineClick", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      repair_psc_finding: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }]);

    document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="fix"]')!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("repair_psc_finding", expect.objectContaining({ line: 1 }));
  });

  it("routes a click on the per-line Ignore button to handleCodeViewerIgnoreLineClick", async () => {
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      add_disable_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);

    document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="ignore"]')!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("add_disable_comment_to_psc_line", expect.objectContaining({ line: 1 }));
  });

  it("routes a click on the per-line File disable button to handleCodeViewerFileDisableLineClick", async () => {
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      add_disable_file_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);

    document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="file-disable"]')!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("add_disable_file_comment_to_psc_line", expect.objectContaining({ line: 1 }));
  });

  it("routes a click on the per-line Config disable button to handleCodeViewerConfigDisableLineClick", async () => {
    invokeImplFor({ read_psc_file: () => "Foo(1,2)\n" });
    await openCodeViewer("/a.psc", [{ line: 1, column: 1, message: "[warning] missing space after comma", rule: "comma-spacing" }]);
    invokeImplFor({
      read_psc_file: () => "Foo(1,2)\n",
      lint_psc_file: () => [],
    });

    document.querySelector<HTMLButtonElement>('#code-viewer-line-1 [data-line-action="config-disable"]')!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", expect.objectContaining({ path: "/a.psc" }));
  });

  it("ignores a click that doesn't land on an action button", async () => {
    invokeImplFor({ read_psc_file: () => "line one\n" });
    await openCodeViewer("/a.psc", []);
    invokeMock.mockReset();

    document.querySelector<HTMLElement>("#code-viewer-line-1")!.click();
    await Promise.resolve();

    expect(invokeMock).not.toHaveBeenCalled();
  });
});

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

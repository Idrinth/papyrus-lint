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
import {
  handleCodeViewerFixClick,
  handleCodeViewerFixLineClick,
  handleCodeViewerIgnoreLineClick,
  handleCodeViewerFileDisableLineClick,
  handleCodeViewerConfigDisableLineClick,
  handleCodeViewerNodiscardLineClick,
} from "./code-viewer-actions";
import { openCodeViewer } from "./code-viewer-dialog";

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

  it("covers only the line's findings that pass the active results-list filters", async () => {
    invokeImplFor({
      read_psc_file: () => "line one  \n",
      add_disable_comment_to_psc_line: () => [],
    });
    await openCodeViewer("/a.psc", [
      { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
      { line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
    ]);
    const warningFilter = document.querySelector<HTMLInputElement>("#filter-warning")!;
    warningFilter.checked = false;
    warningFilter.dispatchEvent(new Event("change"));
    try {
      const button = ignoreLineButton();

      await handleCodeViewerIgnoreLineClick(1, button);

      expect(invokeMock).toHaveBeenCalledWith(
        "add_disable_comment_to_psc_line",
        expect.objectContaining({ rules: ["forbidden-functions"], line: 1 }),
      );
    } finally {
      warningFilter.checked = true;
      warningFilter.dispatchEvent(new Event("change"));
    }
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

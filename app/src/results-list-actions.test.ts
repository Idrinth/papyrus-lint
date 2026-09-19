import { vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args), isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ show: showWindowMock }) }));

import { describe, expect, it } from "vitest";
import { invokeImplFor } from "./test/harness";
import { type Diagnostic, type PscParseOutcome } from "./backend";
import { handleCompileClick, handleFixClick, handleFixIssueClick, handleMassFixClick } from "./results-list-actions";

describe("handleFixClick", () => {
  it("disables the button, applies the repair, and re-renders with updated findings", async () => {
    const remaining: Diagnostic[] = [];
    invokeImplFor({ repair_psc_file: () => remaining });

    const button = document.createElement("button");
    const outcome: PscParseOutcome = {
      path: "/a.psc",
      ok: true,
      detail: "",
      findings: [{ line: 1, column: 1, message: "Line contains trailing whitespace" }],
    };

    const promise = handleFixClick("/a.psc", outcome, button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcome.findings).toEqual(remaining);
  });

  it("reports repair failures without rejecting", async () => {
    invokeMock.mockRejectedValue(new Error("repair failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const button = document.createElement("button");
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "parsed", findings: [] };

    await expect(handleFixClick("/a.psc", outcome, button)).resolves.toBeUndefined();
    expect(button.disabled).toBe(true);
  });
});


describe("handleFixIssueClick", () => {
  function setup(findings: Diagnostic[]) {
    const button = document.createElement("button");
    const errorEl = document.createElement("span");
    errorEl.hidden = true;
    const outcome: PscParseOutcome = { path: "/a.psc", ok: true, detail: "", findings };
    return { button, errorEl, outcome };
  }

  it("disables the button, applies just that finding's fix, and re-renders with updated findings", async () => {
    const finding: Diagnostic = {
      line: 3,
      column: 5,
      message: "[warning] missing space after comma",
      rule: "comma-spacing",
    };
    const remaining: Diagnostic[] = [];
    invokeImplFor({ repair_psc_finding: () => remaining });
    const { button, errorEl, outcome } = setup([finding]);

    const promise = handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcome.findings).toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_finding", expect.objectContaining({
      rule: "comma-spacing",
      line: 3,
    }));
  });

  it("does nothing for a finding with no rule", async () => {
    const finding: Diagnostic = { line: 1, column: 1, message: "[error] bad" };
    const { button, errorEl, outcome } = setup([finding]);

    await handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);

    expect(invokeMock).not.toHaveBeenCalledWith("repair_psc_finding", expect.anything());
    expect(button.disabled).toBe(false);
  });

  it("shows the backend's error inline and re-enables the button instead of re-rendering", async () => {
    const finding: Diagnostic = {
      line: 4,
      column: 1,
      message: "[warning] out of order",
      rule: "property-sorting",
    };
    invokeImplFor({
      repair_psc_finding: () => Promise.reject(new Error('Fixing this issue would change other lines in the file; use "Apply fixes" instead.')),
    });
    const { button, errorEl, outcome } = setup([finding]);
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleFixIssueClick("/a.psc", outcome, finding, button, errorEl);

    expect(outcome.findings).toEqual([finding]);
    expect(button.disabled).toBe(false);
    expect(errorEl.hidden).toBe(false);
    expect(errorEl.textContent).toContain("Apply fixes");
  });
});

describe("handleCompileClick", () => {
  function setup() {
    const button = document.createElement("button");
    button.textContent = "Compile";
    const outputEl = document.createElement("pre");
    outputEl.hidden = true;
    return { button, outputEl };
  }

  it("disables the button while compiling and restores its label afterward", async () => {
    invokeImplFor({ compile_psc_file: () => ({ success: true, stdout: "", stderr: "" }) });
    const { button, outputEl } = setup();

    const promise = handleCompileClick("/a.psc", button, outputEl);
    expect(button.disabled).toBe(true);
    expect(button.textContent).toBe("Compiling…");
    await promise;

    expect(button.disabled).toBe(false);
    expect(button.textContent).toBe("Compile");
  });

  it("shows the compiler's output and marks success", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.\n", stderr: "" }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.hidden).toBe(false);
    expect(outputEl.textContent).toContain("Compilation succeeded.");
    expect(outputEl.classList.contains("psc-result__compile-output--ok")).toBe(true);
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(false);
  });

  it("reports when personal data was stripped from the compiled script", async () => {
    invokeImplFor({
      compile_psc_file: () => ({
        success: true,
        stdout: "Compilation succeeded.\n",
        stderr: "",
        personal_data_stripped: true,
      }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toBe(
      "Compilation succeeded.\n\nRemoved your username/computer name from the compiled script.",
    );
  });

  it("shows a default success message when the compiler produced no output", async () => {
    invokeImplFor({ compile_psc_file: () => ({ success: true, stdout: "", stderr: "" }) });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toBe("Compiled successfully.");
  });

  it("marks a compiler-reported failure and shows its stderr", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: false, stdout: "", stderr: "Broken.psc(3,1): error\n" }),
    });
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toContain("Broken.psc(3,1): error");
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(true);
  });

  it("shows a failure to launch the compiler (e.g. no path configured) as an error", async () => {
    invokeMock.mockRejectedValue(new Error("No PapyrusCompiler.exe path is configured."));
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { button, outputEl } = setup();

    await handleCompileClick("/a.psc", button, outputEl);

    expect(outputEl.textContent).toContain("No PapyrusCompiler.exe path is configured.");
    expect(outputEl.classList.contains("psc-result__compile-output--error")).toBe(true);
    expect(button.disabled).toBe(false);
  });
});

describe("handleMassFixClick", () => {
  const trailingWhitespace: Diagnostic = {
    line: 1,
    column: 1,
    message: "[warning] Line contains trailing whitespace",
    rule: "trailing-whitespace",
  };
  const commaSpacing: Diagnostic = {
    line: 3,
    column: 5,
    message: "[warning] missing space after comma",
    rule: "comma-spacing",
  };

  it("repairs the named rule only in files that have it, leaving the rest untouched", async () => {
    const outcomeA: PscParseOutcome = { path: "/a.psc", ok: false, detail: "", findings: [trailingWhitespace] };
    const outcomeB: PscParseOutcome = {
      path: "/b.psc",
      ok: false,
      detail: "",
      findings: [trailingWhitespace, commaSpacing],
    };
    const outcomeC: PscParseOutcome = { path: "/c.psc", ok: false, detail: "", findings: [commaSpacing] };
    invokeImplFor({
      repair_psc_file_rule: (args) => {
        const { path } = args as { path: string };
        return path === "/a.psc" ? [] : [commaSpacing];
      },
    });

    const button = document.createElement("button");
    const promise = handleMassFixClick("trailing-whitespace", [outcomeA, outcomeB, outcomeC], button);
    expect(button.disabled).toBe(true);
    await promise;

    expect(outcomeA.findings).toEqual([]);
    expect(outcomeB.findings).toEqual([commaSpacing]);
    expect(outcomeC.findings).toEqual([commaSpacing]);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({
      path: "/a.psc",
      rule: "trailing-whitespace",
    }));
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({
      path: "/b.psc",
      rule: "trailing-whitespace",
    }));
    expect(invokeMock).not.toHaveBeenCalledWith("repair_psc_file_rule", expect.objectContaining({ path: "/c.psc" }));
  });
});

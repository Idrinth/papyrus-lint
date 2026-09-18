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

import "./test/harness";
import { formatIssuesAsText } from "./results-export-text";

describe("formatIssuesAsText", () => {
  it("renders one CLI-style diagnostic line per finding, via the format_issues_as_text Tauri command", async () => {
    const text = await formatIssuesAsText([
      {
        path: "scripts/source/A.psc",
        findings: [
          { line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
          { line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" },
        ],
      },
    ]);

    expect(text).toBe(
      "scripts/source/A.psc:1:1: [trailing-whitespace] [warning] trailing whitespace\n" +
        "scripts/source/A.psc:5:3: [forbidden-functions] [error] forbidden function used",
    );
    expect(invokeMock).toHaveBeenCalledWith("format_issues_as_text", {
      files: [
        {
          path: "scripts/source/A.psc",
          findings: [
            { line: 1, column: 1, rule: "trailing-whitespace", message: "[warning] trailing whitespace" },
            { line: 5, column: 3, rule: "forbidden-functions", message: "[error] forbidden function used" },
          ],
        },
      ],
    });
  });

  it("falls back to 'unknown' for a finding with no rule id", async () => {
    const text = await formatIssuesAsText([
      { path: "A.psc", findings: [{ line: 1, column: 1, message: "[error] compiler failure" }] },
    ]);

    expect(text).toBe("A.psc:1:1: [unknown] [error] compiler failure");
  });

  it("returns an empty string for no files", async () => {
    expect(await formatIssuesAsText([])).toBe("");
  });
});

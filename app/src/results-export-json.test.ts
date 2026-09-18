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
import { formatIssuesAsJson } from "./results-export-json";

describe("formatIssuesAsJson", () => {
  it("mirrors the CLI --json report shape, restricted to the given files/findings", async () => {
    const json = await formatIssuesAsJson([
      {
        path: "A.psc",
        findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
      },
      {
        path: "B.psc",
        findings: [
          { line: 5, column: 3, message: "[error] forbidden function used", rule: "forbidden-functions" },
          { line: 6, column: 1, message: "[info] consider renaming" },
        ],
      },
    ]);

    expect(JSON.parse(json)).toEqual({
      files: [
        {
          path: "A.psc",
          diagnostics: [
            {
              line: 1,
              column: 1,
              rule: "trailing-whitespace",
              level: "warning",
              message: "[warning] trailing whitespace",
              doc_url: null,
            },
          ],
          diff: null,
        },
        {
          path: "B.psc",
          diagnostics: [
            {
              line: 5,
              column: 3,
              rule: "forbidden-functions",
              level: "error",
              message: "[error] forbidden function used",
              doc_url: null,
            },
            { line: 6, column: 1, rule: "unknown", level: "info", message: "[info] consider renaming", doc_url: null },
          ],
          diff: null,
        },
      ],
      files_with_diagnostics: 2,
      total_diagnostics: 3,
    });
  });

  it("returns an empty report for no files", async () => {
    expect(JSON.parse(await formatIssuesAsJson([]))).toEqual({
      files: [],
      files_with_diagnostics: 0,
      total_diagnostics: 0,
    });
  });

  it("orders a file's diagnostics by line then column, regardless of the order findings were collected in", async () => {
    const json = await formatIssuesAsJson([
      {
        path: "A.psc",
        findings: [
          { line: 8, column: 3, message: "[error] no viable alternative", rule: "compiler-error" },
          { line: 1, column: 5, message: "[warning] trailing whitespace", rule: "trailing-whitespace" },
          { line: 1, column: 1, message: "[info] consider renaming", rule: "identifier-casing" },
        ],
      },
    ]);

    expect(
      JSON.parse(json).files[0].diagnostics.map((d: { line: number; column: number }) => [d.line, d.column]),
    ).toEqual([
      [1, 1],
      [1, 5],
      [8, 3],
    ]);
  });
});

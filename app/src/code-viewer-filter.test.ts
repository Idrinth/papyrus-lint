import { afterEach, describe, expect, it, vi } from "vitest";
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
import { openCodeViewer } from "./code-viewer-dialog";
import { enterCodeViewerEditMode } from "./live-edit-persist";
import { applyRuleTags } from "./main";

const warningFinding = {
  line: 1,
  column: 1,
  message: "[warning] Line contains trailing whitespace",
  rule: "trailing-whitespace",
};
const errorFinding = {
  line: 2,
  column: 1,
  message: "[error] forbidden function used",
  rule: "forbidden-functions",
};

function setSeverityFilter(severity: "error" | "warning" | "info", checked: boolean) {
  const el = document.querySelector<HTMLInputElement>(`#filter-${severity}`)!;
  el.checked = checked;
  el.dispatchEvent(new Event("change"));
}

describe("code viewer and editor honor results-list filters", () => {
  afterEach(() => {
    setSeverityFilter("error", true);
    setSeverityFilter("warning", true);
    setSeverityFilter("info", true);
    applyRuleTags([]);
  });

  async function openMixedFindings() {
    invokeImplFor({
      read_psc_file: () => "line one  \nline two\n",
    });
    await openCodeViewer("/a.psc", [warningFinding, errorFinding]);
  }

  it("hides filtered-out findings in the read-only viewer", async () => {
    setSeverityFilter("warning", false);
    await openMixedFindings();

    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      false,
    );
    expect(document.querySelector("#code-viewer-line-2")!.classList.contains("code-viewer__line--error")).toBe(true);
    expect(document.querySelector('#code-viewer-line-1 [data-line-action="ignore"]')).toBeNull();
    expect(document.querySelector('#code-viewer-line-2 [data-line-action="ignore"]')).not.toBeNull();
  });

  it("hides the viewer's Apply fixes button when no remaining visible finding is fixable", async () => {
    setSeverityFilter("warning", false);
    await openMixedFindings();

    expect(document.querySelector<HTMLButtonElement>("#code-viewer-fix")!.hidden).toBe(true);
    expect(document.querySelector<HTMLButtonElement>("#code-viewer-preview-fix")!.hidden).toBe(true);
  });

  it("hides filtered-out findings in the editor highlight and accessible label", async () => {
    setSeverityFilter("warning", false);
    await openMixedFindings();
    enterCodeViewerEditMode();

    const lines = document.querySelectorAll("#code-viewer-editor-highlight .code-viewer__editor-line");
    expect(lines[0].classList.contains("code-viewer__line--warning")).toBe(false);
    expect(lines[1].classList.contains("code-viewer__line--error")).toBe(true);
    const label = document.querySelector("#code-viewer-editor-textarea")!.getAttribute("aria-label")!;
    expect(label).not.toContain("[warning] Line contains trailing whitespace");
    expect(label).toContain("[error] forbidden function used");
  });

  it("updates an already-open viewer when the results-list filters change", async () => {
    await openMixedFindings();
    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      true,
    );

    setSeverityFilter("warning", false);

    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      false,
    );
    expect(document.querySelector("#code-viewer-line-2")!.classList.contains("code-viewer__line--error")).toBe(true);
    expect(document.querySelector<HTMLButtonElement>("#code-viewer-fix")!.hidden).toBe(true);
  });

  it("updates an already-open editor when the results-list filters change", async () => {
    await openMixedFindings();
    enterCodeViewerEditMode();

    setSeverityFilter("warning", false);

    const lines = document.querySelectorAll("#code-viewer-editor-highlight .code-viewer__editor-line");
    expect(lines[0].classList.contains("code-viewer__line--warning")).toBe(false);
    expect(lines[1].classList.contains("code-viewer__line--error")).toBe(true);
  });

  it("hides findings whose rule is unchecked in the results-list rule filter", async () => {
    applyRuleTags([
      {
        rule: "trailing-whitespace",
        description: "Test description for trailing whitespace.",
        kinds: ["style"],
        importance: "low",
        auto_fixable: true,
        doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace",
      },
      {
        rule: "forbidden-functions",
        description: "Test description for forbidden functions.",
        kinds: ["correctness"],
        importance: "high",
        auto_fixable: false,
        doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-forbidden-functions",
      },
    ]);
    document.querySelector<HTMLInputElement>("#filter-kind-style")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-kind-style")!.dispatchEvent(new Event("change"));

    try {
      await openMixedFindings();

      expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
        false,
      );
      expect(document.querySelector("#code-viewer-line-2")!.classList.contains("code-viewer__line--error")).toBe(true);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-kind-style")!.dispatchEvent(new Event("change"));
    }
  });
});

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
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { buildPscResultItem } from "./results-list-item";
import { openCodeViewer } from "./code-viewer-dialog";
import { handleDroppedPaths } from "./drop";
import { type RuleTagsInfo, type PscParseOutcome } from "./backend";
import { applyRuleTags, clearError, showError, showResult } from "./main";
import { escapeAttr, levelOf, severityOf } from "./main-severity";
import { switchTab } from "./main-tabs";
describe("severity helpers", () => {
  it("levelOf extracts a recognized bracketed prefix", () => {
    expect(levelOf("[error] boom")).toBe("error");
    expect(levelOf("[warning] hmm")).toBe("warning");
    expect(levelOf("[info] fyi")).toBe("info");
  });

  it("levelOf returns null when there is no recognized prefix", () => {
    expect(levelOf("Line contains trailing whitespace")).toBeNull();
    expect(levelOf("[weird] not a real level")).toBeNull();
  });

  it("severityOf falls back to 'error' when there is no level prefix", () => {
    expect(severityOf("[error] boom")).toBe("error");
    expect(severityOf("no prefix here")).toBe("error");
  });
});

describe("escapeAttr", () => {
  it("escapes &, \", <, > for safe use inside an HTML attribute", () => {
    expect(escapeAttr(`a & b " <c> `)).toBe("a \u0026amp; b \u0026quot; \u0026lt;c\u0026gt; ");
  });
});

describe("switchTab", () => {
  it("shows only the selected panel and marks its tab selected", () => {
    switchTab("settings");

    const importPanel = document.querySelector<HTMLElement>("#panel-import")!;
    const settingsPanel = document.querySelector<HTMLElement>("#panel-settings")!;
    const importTab = document.querySelector<HTMLButtonElement>("#tab-import")!;
    const settingsTab = document.querySelector<HTMLButtonElement>("#tab-settings")!;

    expect(settingsPanel.hidden).toBe(false);
    expect(importPanel.hidden).toBe(true);
    expect(settingsTab.getAttribute("aria-selected")).toBe("true");
    expect(importTab.getAttribute("aria-selected")).toBe("false");
    expect(settingsTab.classList.contains("tabs__tab--active")).toBe(true);
  });
});

describe("applyRuleTags", () => {
  const trailingWhitespaceTags: RuleTagsInfo = {
    rule: "trailing-whitespace",
    description: "Test description for trailing whitespace.",
    kinds: ["style"],
    importance: "low",
    auto_fixable: true,
    doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace",
  };

  it("indexes tags by rule id for tagsForFinding", async () => {
    const { tagsForFinding } = await import("./results-filter");
    applyRuleTags([trailingWhitespaceTags]);

    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toEqual(
      trailingWhitespaceTags,
    );
    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "unknown-rule" })).toBeUndefined();
    expect(tagsForFinding({ line: 1, column: 1, message: "x" })).toBeUndefined();

    // Reset so this doesn't leak into later tests that assume no tags are
    // known; applyRuleTags's own index is module state that outlives
    // mountFixture().
    applyRuleTags([]);
  });
});

describe("showError / clearError / showResult", () => {
  it("showError displays the message, hides results, and switches to the import tab", () => {
    switchTab("lint");
    showError("bad file");

    expect(document.querySelector("#drop-zone-error")!.textContent).toBe("bad file");
    expect(document.querySelector("#achlist-result")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-import")!.hidden).toBe(false);
  });

  it("clearError empties the error message", () => {
    showError("bad file");
    clearError();
    expect(document.querySelector("#drop-zone-error")!.textContent).toBe("");
  });

  it("showResult lists the entries and switches to the files tab", () => {
    showResult("/proj/a.achlist", ["/proj/one.psc", "/proj/two.psc"], "/proj");

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/a.achlist");
    expect(document.querySelectorAll("#achlist-result-list > li")).toHaveLength(2);
    expect(document.querySelector("#achlist-result")!.hasAttribute("hidden")).toBe(false);
    expect(document.querySelector<HTMLElement>("#panel-files")!.hidden).toBe(false);
  });

  it("showResult displays entries relative to the given base directory", () => {
    showResult("/proj/a.achlist", ["/proj/scripts/source/one.psc", "/proj/readme.txt"], "/proj");

    const items = document.querySelectorAll("#achlist-result-list > li span");
    expect(items[0].textContent).toBe("scripts/source/one.psc");
    expect(items[1].textContent).toBe("readme.txt");
  });

  it("showResult falls back to the absolute path when no base directory is known", () => {
    showResult("/proj/a.achlist", ["/proj/one.psc"], null);

    expect(document.querySelector("#achlist-result-list > li span")!.textContent).toBe("/proj/one.psc");
  });

  it("showResult adds a View button only for .psc entries", () => {
    showResult("/proj/a.achlist", ["/proj/one.psc", "/proj/readme.txt"], "/proj");

    const rows = document.querySelectorAll("#achlist-result-list > li");
    expect(rows[0].querySelector(".achlist-result__view-button")).not.toBeNull();
    expect(rows[1].querySelector(".achlist-result__view-button")).toBeNull();
  });

  it("showResult's View button opens the code viewer for that .psc file, marking its current findings", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["one.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "One" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] risky" }],
      read_psc_file: () => 'Debug.Trace("hi")',
    });

    // Populates currentPscOutcomes (via the lint pass) so the View button
    // rendered by showResult below has real findings to look up.
    const pending = handleDroppedPaths(["/proj/a.achlist"]);
    await confirmDetectedConfig();
    await pending;

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();

    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    await vi.waitFor(() => {
      expect(document.querySelector("#code-viewer-line-1")).not.toBeNull();
    });
    expect(dialog.hasAttribute("open")).toBe(true);
    expect(document.querySelector("#code-viewer-title")!.textContent).toBe("one.psc");
    expect(document.querySelector("#code-viewer-line-1")!.classList.contains("code-viewer__line--warning")).toBe(
      true,
    );
  });

  it("showResult's View button opens files that have no current lint outcome", async () => {
    invokeImplFor({ read_psc_file: () => "ScriptName Unlinted" });
    showResult("/proj/a.achlist", ["/proj/unlinted.psc"], "/proj");

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();

    await vi.waitFor(() => {
      expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(true);
    });
    expect(document.querySelector("#code-viewer-title")!.textContent).toMatch(/unlinted\.psc$/);
    expect(document.querySelectorAll(".code-viewer__line--error, .code-viewer__line--warning")).toHaveLength(0);
  });
});

describe("wired DOM interactions", () => {
  it("loads startup metadata and applies theme changes through the registered listeners", async () => {
    const version = document.createElement("span");
    version.id = "app-version";
    document.body.append(version);
    invokeImplFor({
      get_app_version: () => "1.2.3",
      list_rule_tags: () => [],
      list_config_presets: () => [],
    });

    // Re-dispatch startup after installing deterministic backend handlers;
    // mountFixture's initial dispatch intentionally happens before each test
    // has configured the command mock.
    document.dispatchEvent(new Event("DOMContentLoaded", { bubbles: true }));
    await vi.waitFor(() => expect(version.textContent).toBe("v1.2.3"));

    const theme = document.querySelector<HTMLSelectElement>("#theme-select")!;
    theme.value = "dark";
    theme.dispatchEvent(new Event("change"));
    expect(localStorage.getItem("papyrus-lint:theme")).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("closes the configuration picker only when its backdrop is clicked", () => {
    const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
    picker.showModal();

    picker.querySelector(".config-picker__title")!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(picker.open).toBe(true);

    picker.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(picker.open).toBe(false);
  });

  it("drives the tab and settings controls through their registered listeners", async () => {
    invokeMock.mockResolvedValue(undefined);

    document.querySelector<HTMLButtonElement>("#tab-settings")!.click();
    expect(document.querySelector<HTMLElement>("#panel-settings")!.hidden).toBe(false);

    const indentation = document.querySelector<HTMLSelectElement>("#indentation-style")!;
    indentation.value = "spaces";
    indentation.dispatchEvent(new Event("change"));
    expect(document.querySelector<HTMLInputElement>("#indentation-width")!.disabled).toBe(false);

    for (const selector of [
      "#compiler-path",
      "#semicolon-style",
      "#indentation-width",
      "#identifier-casing-style",
      "#named-arguments-style",
      "#cyclomatic-complexity-warning",
      "#cyclomatic-complexity-error",
      "#fail-on-warning",
      "#fail-on-info",
      "#bool-like-int",
      "#assume-auto-properties-filled",
      "#rule-trailing_whitespace",
      "#rule-property_sorting",
    ]) {
      document.querySelector<HTMLElement>(selector)!.dispatchEvent(new Event("change"));
    }
    await Promise.resolve();
  });

  it("handles all drag/drop event variants", async () => {
    const registration = onDragDropEventMock.mock.calls[onDragDropEventMock.mock.calls.length - 1];
    expect(registration).toBeDefined();
    const listener = registration![0] as (event: { payload: { type: string; paths: string[] } }) => void;
    const dropZone = document.querySelector<HTMLElement>("#drop-zone")!;

    listener({ payload: { type: "over", paths: [] } });
    expect(dropZone.classList.contains("drop-zone--active")).toBe(true);
    listener({ payload: { type: "cancel", paths: [] } });
    expect(dropZone.classList.contains("drop-zone--active")).toBe(false);

    invokeImplFor({
      parse_achlist_file: () => [],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    listener({ payload: { type: "drop", paths: ["/proj/list.achlist"] } });
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith("parse_achlist_file", { path: "/proj/list.achlist" }));
  });

  it("wires result action buttons and finding links", async () => {
    const outcome: PscParseOutcome = {
      path: "/a.psc",
      ok: true,
      detail: "parsed",
      findings: [{ line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" }],
    };
    const item = buildPscResultItem(outcome)!;
    document.body.append(item);

    invokeImplFor({
      read_psc_file: () => "Int x = 1 ",
      repair_psc_file: () => [],
      compile_psc_file: () => ({ success: true, stdout: "ok", stderr: "" }),
    });
    item.querySelector<HTMLButtonElement>(".psc-result__view-button")!.click();
    item.querySelector<HTMLElement>(".psc-result__finding")!.click();
    item.querySelector<HTMLButtonElement>(".psc-result__fix-button")!.click();
    item.querySelector<HTMLButtonElement>(".psc-result__compile-button")!.click();

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.anything()));
    await vi.waitFor(() => expect(item.querySelector(".psc-result__compile-output")!.textContent).toBe("ok"));
  });

  it("wires code viewer buttons, input, scrolling, backdrop, cancel, and close", async () => {
    invokeImplFor({ read_psc_file: () => "Int x = 1\n" });
    await openCodeViewer("/a.psc", []);
    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
    const textarea = document.querySelector<HTMLTextAreaElement>("#code-viewer-editor-textarea")!;
    const highlight = document.querySelector<HTMLElement>("#code-viewer-editor-highlight")!;
    const gutter = document.querySelector<HTMLElement>("#code-viewer-editor-gutter")!;

    document.querySelector<HTMLButtonElement>("#code-viewer-edit")!.click();
    textarea.value = "Int x = 2\n";
    textarea.dispatchEvent(new Event("input"));
    textarea.scrollTop = 12;
    textarea.scrollLeft = 7;
    textarea.dispatchEvent(new Event("scroll"));
    expect(highlight.scrollTop).toBe(12);
    expect(highlight.scrollLeft).toBe(7);
    expect(gutter.scrollTop).toBe(12);

    textarea.dispatchEvent(new MouseEvent("mousemove", { clientY: 20 }));
    textarea.dispatchEvent(new Event("scroll"));
    textarea.dispatchEvent(new MouseEvent("mouseleave"));
    expect(textarea.title).toBe("");

    vi.spyOn(window, "confirm").mockReturnValue(false);
    const cancelEvent = new Event("cancel", { cancelable: true });
    dialog.dispatchEvent(cancelEvent);
    expect(cancelEvent.defaultPrevented).toBe(true);
    dialog.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(dialog.open).toBe(true);

    vi.mocked(window.confirm).mockReturnValue(true);
    document.querySelector<HTMLButtonElement>("#code-viewer-cancel")!.click();
    document.querySelector<HTMLButtonElement>("#code-viewer-fullscreen")!.click();
    dialog.dispatchEvent(new Event("close"));
    expect(dialog.classList.contains("code-viewer--fullscreen")).toBe(false);
    document.querySelector<HTMLButtonElement>("#code-viewer-close")!.click();
  });
});

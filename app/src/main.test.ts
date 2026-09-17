import { afterEach, describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import { confirmDetectedConfig, invokeImplFor } from "./test/harness";
import { mountFixture } from "./test/fixture";
import { DEFAULT_LINT_CONFIG, handleLintConfigChanged, type LintConfig } from "./config";
import { handleConfigPathOverrideChanged, useProjectDir } from "./project";
import { buildPscResultItem } from "./results-list";
import { openCodeViewer } from "./code-viewer";
import {
  addDisableCommentToPscLine,
  applyRuleTags,
  applyTheme,
  clearError,
  escapeAttr,
  handleDroppedPaths,
  hasFixableFindings,
  isFixableFinding,
  levelOf,
  loadAppVersion,
  loadRuleTags,
  loadStoredTheme,
  parsePscFiles,
  previewRepairPscFile,
  relintCurrentFiles,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  severityOf,
  showError,
  showResult,
  storeTheme,
  switchTab,
  type Diagnostic,
  type PscParseOutcome,
  type RuleTagsInfo,
} from "./main";

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
    expect(escapeAttr(`a & b " <c> `)).toBe("a &amp; b &quot; &lt;c&gt; ");
  });
});

describe("hasFixableFindings", () => {
  it("is true for trailing whitespace findings", () => {
    expect(
      hasFixableFindings([
        { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
      ]),
    ).toBe(true);
  });

  it("is true for semicolon findings", () => {
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Lines should end with a semicolon", rule: "semicolon" }]),
    ).toBe(true);
  });

  it("is true for indentation findings", () => {
    expect(
      hasFixableFindings([{ line: 1, column: 1, message: "[warning] Wrong indentation", rule: "indentation" }]),
    ).toBe(true);
  });

  it("is false when no findings are auto-fixable", () => {
    expect(hasFixableFindings([{ line: 1, column: 1, message: "[error] forbidden function used" }])).toBe(false);
  });

  it("is false for a fixable rule whose finding notes it has no automatic fix", () => {
    expect(
      hasFixableFindings([
        { line: 1, column: 1, message: "[warning] Bad casing (no automatic fix)", rule: "type-casing" },
      ]),
    ).toBe(false);
  });

  it("is false for an empty findings list", () => {
    expect(hasFixableFindings([])).toBe(false);
  });
});

describe("isFixableFinding", () => {
  it("is true for a finding whose rule has an automatic fix", () => {
    expect(isFixableFinding({ line: 1, column: 1, message: "[warning] trailing", rule: "trailing-whitespace" })).toBe(
      true,
    );
  });

  it("is true for a slow-function finding", () => {
    expect(
      isFixableFinding({ line: 1, column: 1, message: "[info] use the faster call", rule: "slow-functions" }),
    ).toBe(true);
  });

  it("is false for a finding whose rule has no automatic fix", () => {
    expect(
      isFixableFinding({ line: 1, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" }),
    ).toBe(false);
  });

  it("is false for a finding with no rule at all", () => {
    expect(isFixableFinding({ line: 1, column: 1, message: "[error] bad" })).toBe(false);
  });

  it("is true for an unused-import finding, since its fix removes the whole line", () => {
    expect(
      isFixableFinding({
        line: 3,
        column: 1,
        message: "[warning] Import 'Helpers' is never used: none of its Global functions are called unqualified anywhere in this script",
        rule: "unused-import",
      }),
    ).toBe(true);
  });

  it("is false for a type-casing finding its own message says has no automatic fix", () => {
    expect(
      isFixableFinding({
        line: 1,
        column: 1,
        message:
          "[warning] Script name 'IDR__TIF__050000F5' does not follow the configured PascalCase casing (fixing this would rename the script, so no automatic fix is applied)",
        rule: "type-casing",
      }),
    ).toBe(false);
  });

  it("is true for a type-casing finding a letter-casing-only rewrite can fix", () => {
    expect(
      isFixableFinding({
        line: 1,
        column: 1,
        message: "[warning] Script name 'myQuestScript' does not follow the configured PascalCase casing",
        rule: "type-casing",
      }),
    ).toBe(true);
  });
});

describe("theme", () => {
  afterEach(() => {
    document.documentElement.removeAttribute("data-theme");
  });

  it("applyTheme sets data-theme for light/dark and clears it for system", () => {
    applyTheme("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    applyTheme("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    applyTheme("system");
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("loadStoredTheme defaults to system when nothing is stored", () => {
    expect(loadStoredTheme()).toBe("system");
  });

  it("round-trips a stored theme through localStorage", () => {
    storeTheme("dark");
    expect(loadStoredTheme()).toBe("dark");
  });

  it("loadStoredTheme falls back to system for an unrecognized value", () => {
    localStorage.setItem("papyrus-lint:theme", "purple");
    expect(loadStoredTheme()).toBe("system");
  });

  it("loadStoredTheme tolerates a broken localStorage", () => {
    const getItemSpy = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(loadStoredTheme()).toBe("system");
    getItemSpy.mockRestore();
  });

  it("storeTheme tolerates a broken localStorage", () => {
    const setItemSpy = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => storeTheme("dark")).not.toThrow();
    setItemSpy.mockRestore();
  });

  it("initializes the theme select from storage and applies the theme on startup", () => {
    localStorage.setItem("papyrus-lint:theme", "dark");
    mountFixture();
    expect(document.querySelector<HTMLSelectElement>("#theme-select")!.value).toBe("dark");
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });

  it("persists and applies the theme when the select changes", () => {
    mountFixture();
    const select = document.querySelector<HTMLSelectElement>("#theme-select")!;
    select.value = "light";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    expect(localStorage.getItem("papyrus-lint:theme")).toBe("light");
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

describe("loadAppVersion", () => {
  it("returns the backend's reported version", async () => {
    invokeImplFor({ get_app_version: () => "1.2.3" });

    await expect(loadAppVersion()).resolves.toBe("1.2.3");
    expect(invokeMock).toHaveBeenCalledWith("get_app_version");
  });

  it("returns an empty string on failure", async () => {
    invokeMock.mockRejectedValue(new Error("command not found"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadAppVersion()).resolves.toBe("");
  });
});

describe("loadRuleTags / applyRuleTags", () => {
  const trailingWhitespaceTags: RuleTagsInfo = {
    rule: "trailing-whitespace",
    description: "Test description for trailing whitespace.",
    kinds: ["style"],
    importance: "low",
    auto_fixable: true,
    doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace",
  };

  afterEach(() => {
    // ruleTagsByRule is module state that outlives mountFixture(); reset it
    // so it doesn't leak into later tests that assume no tags are known.
    applyRuleTags([]);
  });

  it("returns the backend's reported rule tags", async () => {
    invokeImplFor({ list_rule_tags: () => [trailingWhitespaceTags] });

    await expect(loadRuleTags()).resolves.toEqual([trailingWhitespaceTags]);
    expect(invokeMock).toHaveBeenCalledWith("list_rule_tags");
  });

  it("returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("command not found"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadRuleTags()).resolves.toEqual([]);
  });

  it("applyRuleTags indexes tags by rule id for tagsForFinding", async () => {
    const { tagsForFinding } = await import("./results-list");
    applyRuleTags([trailingWhitespaceTags]);

    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "trailing-whitespace" })).toEqual(
      trailingWhitespaceTags,
    );
    expect(tagsForFinding({ line: 1, column: 1, message: "x", rule: "unknown-rule" })).toBeUndefined();
    expect(tagsForFinding({ line: 1, column: 1, message: "x" })).toBeUndefined();
  });
});

describe("parsePscFiles / repairPscFile", () => {
  it("reports a successful parse with its lint findings", async () => {
    invokeImplFor({
      parse_psc_file: () => ({ name: "MyScript" }),
      lint_psc_file: () => [{ line: 2, column: 1, message: "Line contains trailing whitespace" }],
    });

    const [outcome] = await parsePscFiles(["/scripts/MyScript.psc"]);
    expect(outcome).toEqual({
      path: "/scripts/MyScript.psc",
      ok: true,
      detail: 'parsed as "MyScript"',
      findings: [{ line: 2, column: 1, message: "Line contains trailing whitespace" }],
    });
  });

  it("reports a failed parse with no findings", async () => {
    invokeMock.mockRejectedValue(new Error("syntax error"));

    const [outcome] = await parsePscFiles(["/scripts/Broken.psc"]);
    expect(outcome.ok).toBe(false);
    expect(outcome.detail).toContain("syntax error");
    expect(outcome.findings).toEqual([]);
  });

  it("invokes onOutcome as each file finishes, without waiting for the rest of the batch", async () => {
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => ((args as { path: string }).path === "B.psc" ? pendingB : []),
    });

    const seen: string[] = [];
    const result = parsePscFiles(["A.psc", "B.psc"], (outcome) => seen.push(outcome.path));

    for (let i = 0; i < 5; i++) {
      await Promise.resolve();
    }
    expect(seen).toEqual(["A.psc"]);

    resolveB([]);
    await result;
    expect(seen).toEqual(["A.psc", "B.psc"]);
  });

  it("caps concurrent work at the machine's hardware concurrency instead of starting every file at once", async () => {
    const original = Object.getOwnPropertyDescriptor(navigator, "hardwareConcurrency");
    Object.defineProperty(navigator, "hardwareConcurrency", { value: 2, configurable: true });
    try {
      const started: string[] = [];
      const pendingResolvers = new Map<string, (findings: Diagnostic[]) => void>();
      invokeImplFor({
        parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
        lint_psc_file: (args) => {
          const path = (args as { path: string }).path;
          started.push(path);
          return new Promise<Diagnostic[]>((resolve) => {
            pendingResolvers.set(path, resolve);
          });
        },
      });

      const result = parsePscFiles(["A.psc", "B.psc", "C.psc"]);

      for (let i = 0; i < 5; i++) {
        await Promise.resolve();
      }
      expect([...started].sort()).toEqual(["A.psc", "B.psc"]);

      pendingResolvers.get("A.psc")?.([]);
      for (let i = 0; i < 5; i++) {
        await Promise.resolve();
      }
      expect([...started].sort()).toEqual(["A.psc", "B.psc", "C.psc"]);

      pendingResolvers.get("B.psc")?.([]);
      pendingResolvers.get("C.psc")?.([]);
      const outcomes = await result;
      expect(outcomes.map((outcome) => outcome.path)).toEqual(["A.psc", "B.psc", "C.psc"]);
    } finally {
      if (original) {
        Object.defineProperty(navigator, "hardwareConcurrency", original);
      }
    }
  });

  it("lint_psc_file forwards the currently configured compiler path and compile-check setting", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe",
      load_compile_check: () => true,
      load_script_roots: () => [],
      load_lookup_script_roots: () => ["C:/Skyrim/Data/Scripts/Source"],
      parse_psc_file: () => ({ name: "MyScript" }),
      lint_psc_file: () => [],
    });
    await useProjectDir("/proj");

    await parsePscFiles(["/scripts/MyScript.psc"]);

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", {
      path: "/scripts/MyScript.psc",
      root: "/proj",
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: ["C:/Skyrim/Data/Scripts/Source"],
      compilerPath: "C:\\Tools\\PapyrusCompiler.exe",
      compileCheck: true,
    });
  });

  it("repairPscFile forwards to the repair_psc_file command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_file: () => remaining });

    await expect(repairPscFile("/scripts/MyScript.psc")).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
    });
  });

  it("previewRepairPscFile forwards to the preview_repair_psc_file command", async () => {
    const diff = "--- /scripts/MyScript.psc\n+++ /scripts/MyScript.psc\n@@ -1,1 +1,1 @@\n-old\n+new\n";
    invokeImplFor({ preview_repair_psc_file: () => diff });

    await expect(previewRepairPscFile("/scripts/MyScript.psc")).resolves.toEqual(diff);
    expect(invokeMock).toHaveBeenCalledWith("preview_repair_psc_file", {
      path: "/scripts/MyScript.psc",
      config: expect.anything(),
    });
  });

  it("repairPscFinding forwards the rule and line to the repair_psc_finding command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_finding: () => remaining });

    await expect(repairPscFinding("/scripts/MyScript.psc", "comma-spacing", 3)).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_finding", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rule: "comma-spacing",
      line: 3,
    });
  });

  it("repairPscFileRule forwards the rule, but no line, to the repair_psc_file_rule command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_file_rule: () => remaining });

    await expect(repairPscFileRule("/scripts/MyScript.psc", "trailing-whitespace")).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file_rule", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rule: "trailing-whitespace",
    });
  });

  it("addDisableCommentToPscLine forwards the rules and line to the add_disable_comment_to_psc_line command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ add_disable_comment_to_psc_line: () => remaining });

    await expect(
      addDisableCommentToPscLine("/scripts/MyScript.psc", ["comma-spacing", "trailing-whitespace"], 3),
    ).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("add_disable_comment_to_psc_line", {
      path: "/scripts/MyScript.psc",
      root: expect.any(String),
      config: expect.anything(),
      additionalRoots: expect.anything(),
      lookupRoots: expect.anything(),
      compilerPath: expect.any(String),
      compileCheck: expect.any(Boolean),
      rules: ["comma-spacing", "trailing-whitespace"],
      line: 3,
    });
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
    await Promise.resolve();
    await Promise.resolve();

    const dialog = document.querySelector<HTMLDialogElement>("#code-viewer")!;
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

describe("handleDroppedPaths", () => {
  it("rejects a drop with no .achlist file and more than one file", async () => {
    await handleDroppedPaths(["/scripts/A.psc", "/scripts/B.psc"]);
    expect(document.querySelector("#drop-zone-error")!.textContent).toContain(".achlist");
  });

  it("rejects a drop of a single file that's neither .achlist nor .psc", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => Promise.reject(new Error("not a directory")),
    });

    await handleDroppedPaths(["/scripts/readme.txt"]);

    expect(document.querySelector("#drop-zone-error")!.textContent).toContain(".psc");
  });

  it("recursively scans a dropped directory with no .achlist, loading project config and linting each .psc found", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => ["/proj/scripts/source/A.psc", "/proj/scripts/source/Requiem/B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/scripts/source"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/scripts/source");
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/A.psc" });
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/Requiem/B.psc" });
  });

  it("falls back to the dropped directory itself as project root when no scripts/source pair is found", async () => {
    invokeImplFor({
      list_psc_files_recursively: () => ["/proj/Nested/A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj"]);
    await confirmDetectedConfig();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/proj" });
  });

  it("parses the achlist, loads project config, and lints each .psc entry", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "readme.txt"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe("Loaded /proj/list.achlist");
    // readme.txt isn't a .psc file, so only A.psc should have been linted.
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "A.psc" });
    expect(invokeMock).not.toHaveBeenCalledWith("parse_psc_file", { path: "readme.txt" });
  });

  it("resolves the project root from a resolved script's own position when the achlist itself lives elsewhere", async () => {
    // Users sometimes drop the .achlist somewhere other than the project
    // root (e.g. next to a game's Data directory) while the actual
    // project, including its papyrus-lint.yaml, lives deeper:
    //
    //   /proj/list.achlist
    //   /proj/somefolder/otherfolder/scripts/source/AType.psc
    //   /proj/somefolder/otherfolder/source/scripts/BType.psc
    //
    // The achlist's own parent directory ("/proj") isn't the real project
    // root, so it must instead be found from the resolved scripts' own
    // position under their scripts/source or source/scripts pair.
    invokeImplFor({
      parse_achlist_file: () => [
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
        "/proj/somefolder/otherfolder/source/scripts/BType.psc",
      ],
      find_project_root: () => "/proj/somefolder/otherfolder",
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "AType" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;

    expect(invokeMock).toHaveBeenCalledWith("load_lint_config", { dir: "/proj/somefolder/otherfolder" });
  });

  it("shows an error when the achlist itself fails to parse", async () => {
    invokeMock.mockRejectedValue(new Error("bad json"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await handleDroppedPaths(["/proj/list.achlist"]);

    expect(document.querySelector("#drop-zone-error")!.textContent).toContain("Failed to read");
  });

  it("lints a single dropped .psc file, resolving the project root two directories up", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [],
    });

    const pending = handleDroppedPaths(["/proj/scripts/source/A.psc"]);
    await confirmDetectedConfig();
    await pending;

    expect(document.querySelector("#achlist-result-title")!.textContent).toBe(
      "Loaded /proj/scripts/source/A.psc",
    );
    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "/proj/scripts/source/A.psc" });
  });

  it("clears previous findings before re-rendering, so re-dropping the same achlist can't show stale diagnostics while it reloads", async () => {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] stale finding" }],
    });
    const firstDrop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await firstDrop;

    let resolveLint: (findings: Diagnostic[]) => void = () => {};
    const pendingLint = new Promise<Diagnostic[]>((resolve) => {
      resolveLint = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => pendingLint,
      read_psc_file: () => 'Debug.Trace("hi")',
    });

    const secondDrop = handleDroppedPaths(["/proj/list.achlist"]);
    // Let the re-render happen, but not the re-lint pass, which is still
    // blocked on pendingLint.
    for (let i = 0; i < 5; i++) {
      await Promise.resolve();
    }

    document.querySelector<HTMLButtonElement>(".achlist-result__view-button")!.click();
    await Promise.resolve();
    await Promise.resolve();

    expect(document.querySelectorAll("#code-viewer-view .code-viewer__line--warning")).toHaveLength(0);

    resolveLint([]);
    await secondDrop;
  });

  it("switches to the lint tab and grows the results list as each file finishes, without waiting for the whole achlist", async () => {
    vi.useFakeTimers();
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path === "B.psc"
          ? pendingB
          : [{ line: 1, column: 1, message: "[warning] from A" }],
    });

    switchTab("import");
    const drop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    // Let A's parse+lint pass resolve, but not B's, which is still pending.
    for (let i = 0; i < 30; i++) {
      await Promise.resolve();
    }

    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(1);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.max).toBe(2);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 1 / 2 files");

    resolveB([{ line: 2, column: 1, message: "[warning] from B" }]);
    await drop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(2);
    // The finished progress bar stays visible for a grace period instead of
    // disappearing the instant the last file finishes.
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(2000);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
  });

  it("does not force the user back to the lint tab if they switch away while later files are still being analyzed", async () => {
    let resolveB: (findings: Diagnostic[]) => void = () => {};
    const pendingB = new Promise<Diagnostic[]>((resolve) => {
      resolveB = resolve;
    });
    invokeImplFor({
      parse_achlist_file: () => ["A.psc", "B.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path === "B.psc"
          ? pendingB
          : [{ line: 1, column: 1, message: "[warning] from A" }],
    });

    const drop = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    // Let A's parse+lint pass resolve, but not B's, which is still pending.
    for (let i = 0; i < 30; i++) {
      await Promise.resolve();
    }
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);

    // The user switches away from the Lint results tab while B is still
    // being analyzed in the background.
    switchTab("settings");

    resolveB([{ line: 2, column: 1, message: "[warning] from B" }]);
    await drop;

    // B finishing (and re-rendering the results list) must not have yanked
    // the user back to the Lint results tab.
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-settings")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(2);
  });

  it("ignores a stale drop's straggling outcome once a newer drop has started, instead of mixing it into the newer results", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    let resolveOldLintStarted: () => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    const oldLintStarted = new Promise<void>((resolve) => {
      resolveOldLintStarted = resolve;
    });
    invokeImplFor({
      parse_achlist_file: (args) =>
        (args as { path: string }).path === "/proj/old.achlist" ? ["Old.psc"] : ["New.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => {
        if ((args as { path: string }).path === "Old.psc") {
          resolveOldLintStarted();
          return pendingOldLint;
        }
        return [{ line: 1, column: 1, message: "[warning] from New" }];
      },
    });

    const oldDrop = handleDroppedPaths(["/proj/old.achlist"]);
    await confirmDetectedConfig();
    await oldLintStarted;

    // A second, newer drop starts while the first one is still stuck
    // linting Old.psc. Both achlists resolve to the same "/proj" project
    // directory, already confirmed by oldDrop above, so this one doesn't
    // show the picker again.
    const newDrop = handleDroppedPaths(["/proj/new.achlist"]);
    await newDrop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector("#psc-result-list")!.textContent).toContain("New.psc");

    // The stale drop's lint pass finally finishes; its outcome must not be
    // mixed into the newer drop's already-rendered results.
    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
  });

  it("ignores a stale single-psc drop's outcome after a newer psc drop has finished", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) =>
        (args as { path: string }).path.endsWith("Old.psc")
          ? pendingOldLint
          : [{ line: 1, column: 1, message: "[warning] from New" }],
    });

    const oldDrop = handleDroppedPaths(["/proj/scripts/source/Old.psc"]);
    await confirmDetectedConfig();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    // Both .psc paths resolve to the same "/proj" project directory,
    // already confirmed above, so this one doesn't show the picker again.
    const newDrop = handleDroppedPaths(["/proj/scripts/source/New.psc"]);
    await newDrop;

    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
    expect(document.querySelector("#psc-result-list")!.textContent).toContain("New.psc");

    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
    expect(items[0].textContent).not.toContain("Old.psc");
  });

  it("ignores a stale directory scan's outcome after a newer directory scan has finished", async () => {
    let resolveOldLint: (findings: Diagnostic[]) => void = () => {};
    let resolveOldLintStarted: () => void = () => {};
    const pendingOldLint = new Promise<Diagnostic[]>((resolve) => {
      resolveOldLint = resolve;
    });
    const oldLintStarted = new Promise<void>((resolve) => {
      resolveOldLintStarted = resolve;
    });
    invokeImplFor({
      list_psc_files_recursively: (args) =>
        (args as { path: string }).path === "/proj/old"
          ? ["/proj/old/Old.psc"]
          : ["/proj/new/New.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      parse_psc_file: (args) => ({ name: (args as { path: string }).path }),
      lint_psc_file: (args) => {
        if ((args as { path: string }).path.endsWith("Old.psc")) {
          resolveOldLintStarted();
          return pendingOldLint;
        }
        return [{ line: 1, column: 1, message: "[warning] from New" }];
      },
    });

    const oldDrop = handleDroppedPaths(["/proj/old"]);
    await confirmDetectedConfig();
    await oldLintStarted;

    // "/proj/new" is a different, not-yet-confirmed project directory, so
    // this drop shows its own picker too.
    const newDrop = handleDroppedPaths(["/proj/new"]);
    await confirmDetectedConfig();
    await newDrop;

    resolveOldLint([{ line: 1, column: 1, message: "[warning] from Old" }]);
    await oldDrop;

    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("New.psc");
    expect(items[0].textContent).not.toContain("Old.psc");
  });
});

describe("relintCurrentFiles / Lint results tab settings staleness", () => {
  async function dropOneFileAndSettle() {
    invokeImplFor({
      parse_achlist_file: () => ["A.psc"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      save_lint_config: () => undefined,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] from first pass" }],
    });
    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  }

  it("relintCurrentFiles does nothing when no files are currently loaded", async () => {
    // An achlist with no .psc entries leaves currentPscOutcomes empty,
    // giving a deterministic "nothing loaded" state to start the test from
    // regardless of what an earlier test left behind (module-level state
    // isn't reset between tests).
    invokeImplFor({
      parse_achlist_file: () => ["readme.txt"],
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    const pending = handleDroppedPaths(["/proj/list.achlist"]);
    await confirmDetectedConfig();
    await pending;
    invokeMock.mockClear();

    await relintCurrentFiles();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("clicking the Lint results tab after a settings change clears the list and re-lints the same files", async () => {
    await dropOneFileAndSettle();

    switchTab("settings");
    invokeImplFor({
      save_lint_config: () => undefined,
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[error] from second pass" }],
    });
    handleLintConfigChanged();
    invokeMock.mockClear();

    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    // The results panel is hidden synchronously (the same "empty" signal a
    // fresh drop uses while its own lint pass is still running) before the
    // re-lint pass's async work (parse_psc_file/lint_psc_file) runs.
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(true);
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);

    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    expect(invokeMock).toHaveBeenCalledWith("parse_psc_file", { path: "A.psc" });
    expect(document.querySelector("#psc-result")!.hasAttribute("hidden")).toBe(false);
    const items = document.querySelectorAll("#psc-result-list > li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("from second pass");
  });

  it("clicking the Lint results tab with no settings change since the last lint just switches tabs", async () => {
    await dropOneFileAndSettle();

    switchTab("settings");
    invokeMock.mockClear();

    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    await Promise.resolve();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(document.querySelector<HTMLElement>("#panel-lint")!.hidden).toBe(false);
    expect(document.querySelectorAll("#psc-result-list > li")).toHaveLength(1);
  });

  it("re-marks results stale once a config-path-override reload settles, even if a race relint already ran against the old config", async () => {
    // Regression test: handleConfigPathOverrideChanged sets lintResultsStale
    // before its useProjectDir() reload (which replaces currentLintConfig)
    // has actually finished. If the Lint results tab is clicked in that
    // window, relintCurrentFiles relints against the still-old config and
    // clears the flag - so once the reload finally lands the new config, the
    // displayed results must be re-flagged stale, or nothing ever re-lints
    // them against it.
    await dropOneFileAndSettle();

    let resolveOverrideLoad: (config: LintConfig) => void = () => {};
    const pendingOverrideLoad = new Promise<LintConfig>((resolve) => {
      resolveOverrideLoad = resolve;
    });
    const custom: LintConfig = { ...DEFAULT_LINT_CONFIG, semicolon: true };
    invokeImplFor({
      load_lint_config_from_path: () => pendingOverrideLoad,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
      load_project_info: () => ({ detected_script_roots: [], used_configuration_file: null }),
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] raced" }],
    });
    document.querySelector<HTMLInputElement>("#config-path-override")!.value = "/profiles/strict.yaml";
    const overrideChange = handleConfigPathOverrideChanged();

    // The override's own config load is still pending, but lintResultsStale
    // was already set before it started; switching to the Lint tab now
    // races ahead of it and relints against the config still in effect.
    invokeMock.mockClear();
    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }
    const raceLintCall = invokeMock.mock.calls.find(([command]) => command === "lint_psc_file");
    expect((raceLintCall?.[1] as { config: LintConfig }).config.semicolon).toBe(false);

    // The override's config load now finishes, well after that race relint
    // already cleared lintResultsStale.
    invokeMock.mockClear();
    resolveOverrideLoad(custom);
    await overrideChange;

    invokeImplFor({
      parse_psc_file: () => ({ name: "A" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] under custom config" }],
    });
    document.querySelector<HTMLButtonElement>("#tab-lint")!.click();
    for (let i = 0; i < 10; i++) {
      await Promise.resolve();
    }

    const secondLintCall = invokeMock.mock.calls.find(([command]) => command === "lint_psc_file");
    expect((secondLintCall?.[1] as { config: LintConfig }).config.semicolon).toBe(true);
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

describe("remaining failure and defensive paths", () => {
  it("lintPscFile logs backend failures and returns no diagnostics", async () => {
    invokeMock.mockRejectedValue(new Error("lint failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(import("./main").then(({ lintPscFile }) => lintPscFile("/a.psc"))).resolves.toEqual([]);
  });
});

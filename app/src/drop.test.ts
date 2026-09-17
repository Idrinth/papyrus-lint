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
import { switchTab } from "./main";
import { handleDroppedPaths, parsePscFiles, relintCurrentFiles } from "./drop";
import { type Diagnostic } from "./backend";
import { DEFAULT_LINT_CONFIG, handleLintConfigChanged, type LintConfig } from "./config";
import { handleConfigPathOverrideChanged } from "./project";

describe("parsePscFiles", () => {
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

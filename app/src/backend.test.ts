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
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { useProjectDir } from "./project-settings";
import { addDisableCommentToPscLine, hasFixableFindings, isFixableFinding, lintPscFile, loadAppVersion, loadRuleTags, previewRepairPscFile, repairPscFile, repairPscFileRule, repairPscFinding, type Diagnostic, type RuleTagsInfo } from "./backend";
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

describe("loadRuleTags", () => {
  const trailingWhitespaceTags: RuleTagsInfo = {
    rule: "trailing-whitespace",
    description: "Test description for trailing whitespace.",
    kinds: ["style"],
    importance: "low",
    auto_fixable: true,
    doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace",
  };

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
});

describe("lint/repair command wrappers", () => {
  it("lintPscFile forwards the currently configured compiler path and compile-check setting", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe",
      load_compile_check: () => true,
      load_script_roots: () => [],
      load_lookup_script_roots: () => ["C:/Skyrim/Data/Scripts/Source"],
    });
    await useProjectDir("/proj");

    invokeImplFor({ lint_psc_file: () => [] });
    await lintPscFile("/scripts/MyScript.psc");

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", {
      path: "/scripts/MyScript.psc",
      context: {
        root: "/proj",
        config: expect.anything(),
        additional_roots: expect.anything(),
        lookup_roots: ["C:/Skyrim/Data/Scripts/Source"],
        compiler_path: "C:\\Tools\\PapyrusCompiler.exe",
        compile_check: true,
      },
    });
  });

  it("lintPscFile logs backend failures and returns no diagnostics", async () => {
    invokeMock.mockRejectedValue(new Error("lint failed"));
    vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(lintPscFile("/a.psc")).resolves.toEqual([]);
  });

  it("repairPscFile forwards to the repair_psc_file command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ repair_psc_file: () => remaining });

    await expect(repairPscFile("/scripts/MyScript.psc")).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", {
      path: "/scripts/MyScript.psc",
      context: {
        root: expect.any(String),
        config: expect.anything(),
        additional_roots: expect.anything(),
        lookup_roots: expect.anything(),
        compiler_path: expect.any(String),
        compile_check: expect.any(Boolean),
      },
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
      context: {
        root: expect.any(String),
        config: expect.anything(),
        additional_roots: expect.anything(),
        lookup_roots: expect.anything(),
        compiler_path: expect.any(String),
        compile_check: expect.any(Boolean),
      },
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
      context: {
        root: expect.any(String),
        config: expect.anything(),
        additional_roots: expect.anything(),
        lookup_roots: expect.anything(),
        compiler_path: expect.any(String),
        compile_check: expect.any(Boolean),
      },
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
      context: {
        root: expect.any(String),
        config: expect.anything(),
        additional_roots: expect.anything(),
        lookup_roots: expect.anything(),
        compiler_path: expect.any(String),
        compile_check: expect.any(Boolean),
      },
      rules: ["comma-spacing", "trailing-whitespace"],
      line: 3,
    });
  });
});

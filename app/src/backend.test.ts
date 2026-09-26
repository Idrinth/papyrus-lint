import { describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
const channelState = vi.hoisted(() => ({
  instances: [] as { onmessage: ((message: unknown) => void) | null }[],
  throwOnConstruct: false,
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
  Channel: class {
    onmessage: ((message: unknown) => void) | null = null;

    constructor() {
      if (channelState.throwOnConstruct) {
        throw new Error("channels unavailable");
      }
      channelState.instances.push(this);
    }
  },
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
import { type CompileOutcome, type Diagnostic, type RuleTagsInfo } from "./backend-types";
import {
  addDisableCommentToPscLine,
  addDisableFileCommentToPscLine,
  addNodiscardCommentToPscLine,
  compilePscFile,
  getPscFileMtimes,
  lintPapyrusScript,
  lintProjectScripts,
  lintPscFile,
  listScriptMembers,
  loadAppVersion,
  loadRuleTags,
  preloadProjectScripts,
  previewRepairPscFile,
  previewRepairPscLine,
  repairPscFile,
  repairPscFileRule,
  repairPscFinding,
  resolveCompletionQuery,
  writePscFile,
} from "./backend";
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

  it("normalizes a null backend response to an empty array", async () => {
    invokeImplFor({ list_rule_tags: () => null });

    await expect(loadRuleTags()).resolves.toEqual([]);
  });
});

describe("lint/repair command wrappers", () => {
  it("lintPapyrusScript forwards source and config", async () => {
    const findings: Diagnostic[] = [{ line: 2, column: 4, message: "[warning] test" }];
    invokeImplFor({ lint_papyrus_script: () => findings });

    await expect(lintPapyrusScript("ScriptName Test")).resolves.toEqual(findings);
    expect(invokeMock).toHaveBeenCalledWith("lint_papyrus_script", {
      source: "ScriptName Test",
      config: expect.anything(),
    });
  });

  it("lintPapyrusScript logs backend failures and returns no diagnostics", async () => {
    const error = new Error("lint failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(lintPapyrusScript("broken")).resolves.toEqual([]);
    expect(consoleError).toHaveBeenCalledWith(error);
  });

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

  it("previewRepairPscLine forwards its target and returns the repaired line", async () => {
    invokeImplFor({ preview_repair_psc_line: () => "  Return value" });

    await expect(previewRepairPscLine("/scripts/MyScript.psc", "indentation", 7)).resolves.toBe("  Return value");
    expect(invokeMock).toHaveBeenCalledWith("preview_repair_psc_line", {
      path: "/scripts/MyScript.psc",
      config: expect.anything(),
      rule: "indentation",
      line: 7,
    });
  });

  it("previewRepairPscLine logs backend failures and returns null", async () => {
    const error = new Error("preview failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(previewRepairPscLine("/scripts/MyScript.psc", "indentation", 7)).resolves.toBeNull();
    expect(consoleError).toHaveBeenCalledWith(error);
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

  it("addDisableFileCommentToPscLine forwards the rules and line to the add_disable_file_comment_to_psc_line command", async () => {
    const remaining: Diagnostic[] = [{ line: 1, column: 1, message: "[error] still broken" }];
    invokeImplFor({ add_disable_file_comment_to_psc_line: () => remaining });

    await expect(
      addDisableFileCommentToPscLine("/scripts/MyScript.psc", ["comma-spacing", "trailing-whitespace"], 3),
    ).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("add_disable_file_comment_to_psc_line", {
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

  it("addNodiscardCommentToPscLine forwards the line to the add_nodiscard_comment_to_psc_line command", async () => {
    const remaining: Diagnostic[] = [];
    invokeImplFor({ add_nodiscard_comment_to_psc_line: () => remaining });

    await expect(addNodiscardCommentToPscLine("/scripts/MyScript.psc", 12)).resolves.toEqual(remaining);
    expect(invokeMock).toHaveBeenCalledWith("add_nodiscard_comment_to_psc_line", {
      path: "/scripts/MyScript.psc",
      context: expect.anything(),
      line: 12,
    });
  });
});

describe("project batch wrappers", () => {
  it("preloadProjectScripts streams progress and releases its channel handler", async () => {
    const onProgress = vi.fn();
    invokeImplFor({
      preload_project_scripts: (args) => {
        const channel = (args as { onProgress: { onmessage: (progress: unknown) => void } }).onProgress;
        channel.onmessage({ phase: "Parsing", completed: 1, total: 2 });
      },
    });

    await preloadProjectScripts(["/a.psc", "/b.psc"], onProgress);

    expect(onProgress).toHaveBeenCalledWith({ phase: "Parsing", completed: 1, total: 2 });
    expect(invokeMock).toHaveBeenCalledWith("preload_project_scripts", {
      paths: ["/a.psc", "/b.psc"],
      context: expect.anything(),
      onProgress: channelState.instances[channelState.instances.length - 1],
    });
    onProgress.mockClear();
    channelState.instances[channelState.instances.length - 1]?.onmessage?.({ phase: "late", completed: 2, total: 2 });
    expect(onProgress).not.toHaveBeenCalled();
  });

  it("preloadProjectScripts still invokes without a channel and logs failures", async () => {
    channelState.throwOnConstruct = true;
    const error = new Error("preload failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(preloadProjectScripts(["/a.psc"])).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("preload_project_scripts", {
      paths: ["/a.psc"],
      context: expect.anything(),
    });
    expect(consoleError).toHaveBeenCalledWith(error);
    channelState.throwOnConstruct = false;
  });

  it("lintProjectScripts forwards streamed events without replaying returned outcomes", async () => {
    const onEvent = vi.fn();
    const event = { kind: "progress" as const, phase: "Linting", completed: 1, total: 1 };
    invokeImplFor({
      lint_project_scripts: (args) => {
        const channel = (args as { onEvent: { onmessage: (value: unknown) => void } }).onEvent;
        channel.onmessage(event);
        return [{ path: "/duplicate.psc", ok: true, detail: "", findings: [] }];
      },
    });

    await lintProjectScripts(["/a.psc"], onEvent);

    expect(onEvent).toHaveBeenCalledExactlyOnceWith(event);
    onEvent.mockClear();
    channelState.instances[channelState.instances.length - 1]?.onmessage?.(event);
    expect(onEvent).not.toHaveBeenCalled();
  });

  it("lintProjectScripts converts returned outcomes and ignores malformed entries", async () => {
    const onEvent = vi.fn();
    const result = { kind: "result" as const, path: "/a.psc", ok: true, detail: "ok", findings: [] };
    invokeImplFor({
      lint_project_scripts: () => [null, "bad", { ok: false }, result, { path: "/b.psc", ok: false, detail: "bad", findings: [] }],
    });

    await lintProjectScripts(["/a.psc", "/b.psc"], onEvent);

    expect(onEvent).toHaveBeenNthCalledWith(1, result);
    expect(onEvent).toHaveBeenNthCalledWith(2, {
      kind: "result",
      path: "/b.psc",
      ok: false,
      detail: "bad",
      findings: [],
    });
  });

  it("lintProjectScripts logs backend failures", async () => {
    const error = new Error("batch failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(lintProjectScripts(["/a.psc"])).resolves.toBeUndefined();
    expect(consoleError).toHaveBeenCalledWith(error);
  });
});

describe("file, completion, and compiler wrappers", () => {
  it("writes files through the backend", async () => {
    invokeImplFor({ write_psc_file: () => undefined });

    await writePscFile("/a.psc", "ScriptName A");
    expect(invokeMock).toHaveBeenCalledWith("write_psc_file", { path: "/a.psc", contents: "ScriptName A" });
  });

  it("returns file modification times and falls back to an empty map on failure", async () => {
    invokeImplFor({ get_psc_file_mtimes: () => ({ "/a.psc": 123 }) });
    await expect(getPscFileMtimes(["/a.psc"])).resolves.toEqual({ "/a.psc": 123 });
    expect(invokeMock).toHaveBeenCalledWith("get_psc_file_mtimes", { paths: ["/a.psc"] });

    const error = new Error("stat failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(getPscFileMtimes(["/missing.psc"])).resolves.toEqual({});
    expect(consoleError).toHaveBeenCalledWith(error);
  });

  it("lists script members with the current project roots", async () => {
    const members = [{ name: "Enable", kind: "function" as const }];
    invokeImplFor({ list_script_members: () => members });

    await expect(listScriptMembers("ObjectReference")).resolves.toEqual(members);
    expect(invokeMock).toHaveBeenCalledWith("list_script_members", {
      root: expect.any(String),
      typeName: "ObjectReference",
      additionalRoots: expect.any(Array),
      lookupRoots: expect.any(Array),
    });
  });

  it("listScriptMembers logs failures and returns no members", async () => {
    const error = new Error("lookup failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(listScriptMembers("MissingType")).resolves.toEqual([]);
    expect(consoleError).toHaveBeenCalledWith(error);
  });

  it("resolves completion queries and returns null on failure", async () => {
    const query = { receiverType: "Actor", prefix: "Get", prefixStart: 8 };
    invokeImplFor({ resolve_completion_query: () => query });
    await expect(resolveCompletionQuery("actor.Get", 9)).resolves.toEqual(query);
    expect(invokeMock).toHaveBeenCalledWith("resolve_completion_query", { source: "actor.Get", cursorIndex: 9 });

    const error = new Error("parse failed");
    invokeMock.mockRejectedValue(error);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(resolveCompletionQuery("broken", 6)).resolves.toBeNull();
    expect(consoleError).toHaveBeenCalledWith(error);
  });

  it("compiles a file with the current game, compiler, and roots", async () => {
    const outcome: CompileOutcome = {
      success: true,
      stdout: "compiled",
      stderr: "",
      personal_data_stripped: false,
    };
    invokeImplFor({ compile_psc_file: () => outcome });

    await expect(compilePscFile("/a.psc")).resolves.toEqual(outcome);
    expect(invokeMock).toHaveBeenCalledWith("compile_psc_file", {
      path: "/a.psc",
      game: expect.any(String),
      compilerPath: expect.any(String),
      additionalRoots: expect.any(Array),
    });
  });
});

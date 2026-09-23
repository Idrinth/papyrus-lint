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
import { applyProjectInfoToUI } from "./project-settings";
import {
  loadCompileCheck,
  loadCompilerPath,
  loadLookupScriptRoots,
  loadProjectInfo,
  loadScriptRoots,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPpj,
  projectDirForPscPath,
  saveCompileCheck,
  saveCompilerPath,
  saveLookupScriptRoots,
  saveScriptRoots,
} from "./project-io";

describe("projectDirForAchlist / projectDirForDirectory / projectDirForPscPath", () => {
  it("projectDirForAchlist asks the backend with only the .psc entries and the achlist's own directory as fallback", async () => {
    invokeImplFor({ find_project_root: () => "/proj/somefolder/otherfolder" });

    await expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/readme.txt",
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
      ]),
    ).resolves.toBe("/proj/somefolder/otherfolder");
    expect(invokeMock).toHaveBeenCalledWith("find_project_root", {
      entries: ["/proj/somefolder/otherfolder/scripts/source/AType.psc"],
      fallback: "/proj",
    });
  });

  it("projectDirForAchlist falls back to the achlist's own directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      projectDirForAchlist("/proj/list.achlist", ["/proj/other/A.psc"]),
    ).resolves.toBe("/proj");
  });

  it("projectDirForPpj asks the backend with only the .psc entries and the ppj's own directory as fallback", async () => {
    invokeImplFor({ find_project_root: () => "/proj/somefolder/otherfolder" });

    await expect(
      projectDirForPpj("/proj/project.ppj", [
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
      ]),
    ).resolves.toBe("/proj/somefolder/otherfolder");
    expect(invokeMock).toHaveBeenCalledWith("find_project_root", {
      entries: ["/proj/somefolder/otherfolder/scripts/source/AType.psc"],
      fallback: "/proj",
    });
  });

  it("projectDirForPpj falls back to the ppj's own directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      projectDirForPpj("/proj/project.ppj", ["/proj/other/A.psc"]),
    ).resolves.toBe("/proj");
  });

  it("projectDirForDirectory asks the backend with every entry and the dropped directory as fallback", async () => {
    invokeImplFor({ find_project_root: () => "/proj" });

    await expect(
      projectDirForDirectory("/proj/scripts/source", [
        "/proj/scripts/source/Requiem/A.psc",
      ]),
    ).resolves.toBe("/proj");
    expect(invokeMock).toHaveBeenCalledWith("find_project_root", {
      entries: ["/proj/scripts/source/Requiem/A.psc"],
      fallback: "/proj/scripts/source",
    });
  });

  it("projectDirForDirectory falls back to the dropped directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      projectDirForDirectory("/proj", ["/proj/Nested/A.psc"]),
    ).resolves.toBe("/proj");
  });

  it("projectDirForPscPath asks the backend for the bare .psc file's project root", async () => {
    invokeImplFor({ find_psc_project_root_for_path: () => "/proj" });

    await expect(
      projectDirForPscPath("/proj/scripts/source/User/A.psc"),
    ).resolves.toBe("/proj");
    expect(invokeMock).toHaveBeenCalledWith("find_psc_project_root_for_path", {
      path: "/proj/scripts/source/User/A.psc",
    });
  });

  it("projectDirForPscPath falls back to two directories above the script's own directory when the backend call fails", async () => {
    invokeMock.mockRejectedValue(new Error("boom"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      projectDirForPscPath("/proj/scripts/source/A.psc"),
    ).resolves.toBe("/proj");
  });
});

describe("loadCompilerPath / saveCompilerPath", () => {
  it("loadCompilerPath returns the backend's resolved path", async () => {
    invokeImplFor({
      load_compiler_path: () => "C:\\Tools\\PapyrusCompiler.exe",
    });

    await expect(loadCompilerPath("/proj")).resolves.toBe(
      "C:\\Tools\\PapyrusCompiler.exe",
    );
    expect(invokeMock).toHaveBeenCalledWith("load_compiler_path", {
      dir: "/proj",
    });
  });

  it("loadCompilerPath returns an empty string when the backend has none", async () => {
    invokeImplFor({ load_compiler_path: () => null });

    await expect(loadCompilerPath("/proj")).resolves.toBe("");
  });

  it("loadCompilerPath returns an empty string on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadCompilerPath("/proj")).resolves.toBe("");
  });

  it("saveCompilerPath swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      saveCompilerPath("/proj", "C:\\Tools\\PapyrusCompiler.exe"),
    ).resolves.toBeUndefined();
  });
});

describe("loadCompileCheck / saveCompileCheck", () => {
  it("loadCompileCheck returns the backend's stored setting", async () => {
    invokeImplFor({ load_compile_check: () => true });

    await expect(loadCompileCheck("/proj")).resolves.toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("load_compile_check", {
      dir: "/proj",
    });
  });

  it("loadCompileCheck returns false on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadCompileCheck("/proj")).resolves.toBe(false);
  });

  it("saveCompileCheck forwards the setting to the backend", async () => {
    invokeImplFor({ save_compile_check: () => undefined });

    await expect(saveCompileCheck("/proj", true)).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_compile_check", {
      dir: "/proj",
      enabled: true,
    });
  });

  it("saveCompileCheck swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(saveCompileCheck("/proj", true)).resolves.toBeUndefined();
  });
});

describe("loadScriptRoots / saveScriptRoots", () => {
  it("loadScriptRoots returns the backend's configured roots", async () => {
    invokeImplFor({
      load_script_roots: () => ["../SharedScripts", "/abs/OtherScripts"],
    });

    await expect(loadScriptRoots("/proj")).resolves.toEqual([
      "../SharedScripts",
      "/abs/OtherScripts",
    ]);
    expect(invokeMock).toHaveBeenCalledWith("load_script_roots", {
      dir: "/proj",
    });
  });

  it("loadScriptRoots returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadScriptRoots("/proj")).resolves.toEqual([]);
  });

  it("saveScriptRoots forwards the roots to the backend", async () => {
    invokeImplFor({ save_script_roots: () => undefined });

    await expect(
      saveScriptRoots("/proj", ["../SharedScripts"]),
    ).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_script_roots", {
      dir: "/proj",
      roots: ["../SharedScripts"],
    });
  });

  it("saveScriptRoots swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      saveScriptRoots("/proj", ["../SharedScripts"]),
    ).resolves.toBeUndefined();
  });
});

describe("loadLookupScriptRoots / saveLookupScriptRoots", () => {
  it("loadLookupScriptRoots returns the backend's configured roots", async () => {
    invokeImplFor({
      load_lookup_script_roots: () => [
        "C:/Skyrim/Data/Scripts/Source",
        "C:/Skyrim/Data/Source/Scripts",
      ],
    });

    await expect(loadLookupScriptRoots("/proj")).resolves.toEqual([
      "C:/Skyrim/Data/Scripts/Source",
      "C:/Skyrim/Data/Source/Scripts",
    ]);
    expect(invokeMock).toHaveBeenCalledWith("load_lookup_script_roots", {
      dir: "/proj",
    });
  });

  it("loadLookupScriptRoots returns an empty array on failure", async () => {
    invokeMock.mockRejectedValue(new Error("no such file"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(loadLookupScriptRoots("/proj")).resolves.toEqual([]);
  });

  it("saveLookupScriptRoots forwards the roots to the backend", async () => {
    invokeImplFor({ save_lookup_script_roots: () => undefined });

    await expect(
      saveLookupScriptRoots("/proj", ["C:/Skyrim/Data/Scripts/Source"]),
    ).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("save_lookup_script_roots", {
      dir: "/proj",
      roots: ["C:/Skyrim/Data/Scripts/Source"],
    });
  });

  it("saveLookupScriptRoots swallows backend errors", async () => {
    invokeMock.mockRejectedValue(new Error("disk full"));
    vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      saveLookupScriptRoots("/proj", ["C:/Skyrim/Data/Scripts/Source"]),
    ).resolves.toBeUndefined();
  });
});

describe("loadProjectInfo / applyProjectInfoToUI", () => {
  it("loads project information from the backend", async () => {
    const info = {
      detected_script_roots: ["/project/scripts/source"],
      used_configuration_file: "/project/papyrus-lint.yaml",
    };
    invokeImplFor({ load_project_info: () => info });

    await expect(loadProjectInfo("/project")).resolves.toEqual(info);
    expect(invokeMock).toHaveBeenCalledWith("load_project_info", {
      dir: "/project",
    });
  });

  it("shows explicit empty-state messages", () => {
    applyProjectInfoToUI({
      detected_script_roots: [],
      used_configuration_file: null,
    });

    expect(document.querySelector("#detected-script-roots")!.textContent).toBe(
      "None detected",
    );
    expect(
      document.querySelector("#used-configuration-file")!.textContent,
    ).toBe("None (using defaults)");
  });
});

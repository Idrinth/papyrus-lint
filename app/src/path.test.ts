import { describe, expect, it } from "vitest";
import {
  dirnameOf,
  findCandidatePairRoot,
  isAchlistPath,
  isPscPath,
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPscPath,
  relativePath,
  scriptRootsForAchlist,
} from "./path";

describe("path helpers", () => {
  it("isAchlistPath matches .achlist regardless of case", () => {
    expect(isAchlistPath("C:/mods/list.achlist")).toBe(true);
    expect(isAchlistPath("C:/mods/list.ACHLIST")).toBe(true);
    expect(isAchlistPath("C:/mods/list.psc")).toBe(false);
  });

  it("isPscPath matches .psc regardless of case", () => {
    expect(isPscPath("Foo.psc")).toBe(true);
    expect(isPscPath("Foo.PSC")).toBe(true);
    expect(isPscPath("Foo.achlist")).toBe(false);
  });

  it("dirnameOf strips the final path component for both slash styles", () => {
    expect(dirnameOf("/a/b/c.achlist")).toBe("/a/b");
    expect(dirnameOf("C:\\a\\b\\c.achlist")).toBe("C:\\a\\b");
  });

  it("dirnameOf returns the whole path when there is no separator", () => {
    expect(dirnameOf("c.achlist")).toBe("c.achlist");
  });

  it("relativePath strips a matching base prefix, for either slash style", () => {
    expect(relativePath("/proj/scripts/A.psc", "/proj")).toBe("scripts/A.psc");
    expect(relativePath("C:\\proj\\scripts\\A.psc", "C:\\proj")).toBe("scripts\\A.psc");
  });

  it("relativePath falls back to the absolute path when base is unknown or unrelated", () => {
    expect(relativePath("/a.psc", null)).toBe("/a.psc");
    expect(relativePath("/elsewhere/A.psc", "/proj")).toBe("/elsewhere/A.psc");
  });
});

describe("projectDirForPscPath", () => {
  it("resolves the project root two directories above the script's own directory", () => {
    expect(projectDirForPscPath("/proj/scripts/source/A.psc")).toBe("/proj");
    expect(projectDirForPscPath("C:\\proj\\scripts\\source\\A.psc")).toBe("C:\\proj");
  });
});

describe("findCandidatePairRoot", () => {
  it("finds the root above a scripts/source pair", () => {
    expect(findCandidatePairRoot("/proj/scripts/source/A.psc")).toBe("/proj");
  });

  it("finds the root above a source/scripts pair", () => {
    expect(findCandidatePairRoot("/proj/source/scripts/A.psc")).toBe("/proj");
  });

  it("matches case-insensitively and across backslash paths", () => {
    expect(findCandidatePairRoot("C:\\proj\\Scripts\\Source\\A.psc")).toBe("C:\\proj");
  });

  it("finds the root for a script nested under a namespaced subfolder", () => {
    expect(findCandidatePairRoot("/proj/scripts/source/User/A.psc")).toBe("/proj");
  });

  it("returns null when no scripts/source or source/scripts pair is present", () => {
    expect(findCandidatePairRoot("/proj/other/A.psc")).toBeNull();
  });
});

describe("projectDirForAchlist", () => {
  it("resolves the conventional layout where the achlist already lives in the project root", () => {
    expect(projectDirForAchlist("/proj/list.achlist", ["/proj/scripts/source/A.psc"])).toBe("/proj");
  });

  it("falls back to a resolved entry's own scripts/source position when the achlist lives elsewhere", () => {
    expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/somefolder/otherfolder/scripts/source/AType.psc",
        "/proj/somefolder/otherfolder/source/scripts/BType.psc",
      ]),
    ).toBe("/proj/somefolder/otherfolder");
  });

  it("falls back to the achlist's parent directory when no entry matches the convention", () => {
    expect(projectDirForAchlist("/proj/list.achlist", ["/proj/other/A.psc"])).toBe("/proj");
  });

  it("ignores non-.psc entries when looking for a matching script position", () => {
    expect(
      projectDirForAchlist("/proj/list.achlist", [
        "/proj/readme.txt",
        "/proj/somefolder/scripts/source/A.psc",
      ]),
    ).toBe("/proj/somefolder");
  });
});

describe("projectDirForDirectory", () => {
  it("resolves a nested scripts/source pair beneath the dropped directory", () => {
    expect(
      projectDirForDirectory("/proj/scripts/source", ["/proj/scripts/source/Requiem/A.psc"]),
    ).toBe("/proj");
  });

  it("falls back to the dropped directory itself when no entry matches the convention", () => {
    expect(projectDirForDirectory("/proj", ["/proj/Nested/A.psc"])).toBe("/proj");
  });

  it("falls back to the dropped directory itself for an empty scan", () => {
    expect(projectDirForDirectory("/proj", [])).toBe("/proj");
  });
});

describe("scriptRootsForAchlist", () => {
  it("uses every distinct directory containing a listed Papyrus source", () => {
    expect(
      scriptRootsForAchlist([
        "/proj/source/dir/one/Script1.psc",
        "/proj/source/dir/two/Script2.PSC",
        "/proj/source/dir/one/Script3.psc",
        "/proj/readme.txt",
      ]),
    ).toEqual(["/proj/source/dir/one", "/proj/source/dir/two"]);
  });
});

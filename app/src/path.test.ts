import { describe, expect, it } from "vitest";
import { dirnameOf, isAchlistPath, isPpjPath, isPscPath, relativePath, scriptRootsForAchlist } from "./path";

describe("path helpers", () => {
  it("isAchlistPath matches .achlist regardless of case", () => {
    expect(isAchlistPath("C:/mods/list.achlist")).toBe(true);
    expect(isAchlistPath("C:/mods/list.ACHLIST")).toBe(true);
    expect(isAchlistPath("C:/mods/list.psc")).toBe(false);
  });

  it("isPpjPath matches .ppj regardless of case", () => {
    expect(isPpjPath("C:/mods/project.ppj")).toBe(true);
    expect(isPpjPath("C:/mods/project.PPJ")).toBe(true);
    expect(isPpjPath("C:/mods/list.achlist")).toBe(false);
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

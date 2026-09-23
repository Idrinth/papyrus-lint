import { invoke } from "@tauri-apps/api/core";
import { type ProjectInfo } from "./backend-types";
import { dirnameOf, isPscPath } from "./path";
export async function loadProjectInfo(dir: string): Promise<ProjectInfo> {
  try {
    return await invoke<ProjectInfo>("load_project_info", { dir });
  } catch (error) {
    console.error(error);
    return { detected_script_roots: [], used_configuration_file: null };
  }
}

// Finds the project root implied by `entries`, delegating to the same
// `scripts/source`/`source/scripts`-pair algorithm the CLI uses (see
// papyrus_lint_core::project_root::find_candidate_pair_root), so the
// desktop app resolves the same root as the CLI would for the same files.
// Falls back to `fallback` if none of `entries` match that layout, or if
// the lookup itself fails.
async function findProjectRoot(entries: string[], fallback: string): Promise<string> {
  try {
    return await invoke<string>("find_project_root", { entries, fallback });
  } catch (error) {
    console.error(error);
    return fallback;
  }
}

// Finds the project root for a dropped `.achlist`: tries each of its
// resolved `.psc` entries' own position under a `scripts/source`/
// `source/scripts` directory pair first (see findProjectRoot), so a
// project whose `.achlist` doesn't live in the project root itself (e.g. it
// was dropped next to a game's `Data` directory while the project itself
// lives in a subfolder) still resolves correctly. Falls back to the
// achlist's own parent directory (the conventional layout) if none of its
// entries match.
export async function projectDirForAchlist(achlistPath: string, entries: string[]): Promise<string> {
  return findProjectRoot(entries.filter(isPscPath), dirnameOf(achlistPath));
}

// Finds the project root for a dropped `.ppj`: tries each of its resolved
// `.psc` scripts' own position under a `scripts/source`/`source/scripts`
// directory pair first (see findProjectRoot), the same way
// projectDirForAchlist does for an achlist's entries. Falls back to the
// ppj's own parent directory (the conventional layout) if none of its
// scripts match.
export async function projectDirForPpj(ppjPath: string, scripts: string[]): Promise<string> {
  return findProjectRoot(scripts.filter(isPscPath), dirnameOf(ppjPath));
}

// Finds the project root for a dropped directory (see handleDroppedPaths'
// directory-scan mode, for a project with no .achlist at all whose scripts
// are spread across arbitrarily nested subfolders, e.g. Requiem's own
// layout): tries each recursively-found .psc entry's own position under a
// `scripts/source`/`source/scripts` directory pair first (see
// findProjectRoot), the same way projectDirForAchlist does for an
// achlist's entries. Falls back to the dropped directory itself if none of
// the entries match that layout, since there's no achlist file whose parent
// directory would otherwise apply.
export async function projectDirForDirectory(dirPath: string, entries: string[]): Promise<string> {
  return findProjectRoot(entries, dirPath);
}

// Finds the project root for a bare `.psc` file dropped directly, mirroring
// the CLI's own handling of a `.psc` path given directly on the command
// line (see papyrus_lint_core::project_root::find_psc_project_root): tries
// the file's own position under a `scripts/source`/`source/scripts` pair
// first, falling back to two directories above its own directory (e.g.
// `Data/Scripts/Source/abc.psc` under `Data`) if it isn't under one.
export async function projectDirForPscPath(pscPath: string): Promise<string> {
  try {
    return await invoke<string>("find_psc_project_root_for_path", { path: pscPath });
  } catch (error) {
    console.error(error);
    return dirnameOf(dirnameOf(dirnameOf(pscPath)));
  }
}

// Returns the PapyrusCompiler.exe path to use for `dir`'s project: an
// explicit override saved to its papyrus-lint config file, or, absent
// one, a path auto-detected at `../Papyrus Compiler/PapyrusCompiler.exe`
// relative to `dir`. Returns an empty string if neither is available or
// the lookup fails.
export async function loadCompilerPath(dir: string): Promise<string> {
  try {
    return (await invoke<string | null>("load_compiler_path", { dir })) ?? "";
  } catch (error) {
    console.error(error);
    return "";
  }
}

// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
// papyrus-lint config file. Passing an empty string clears the override,
// reverting to auto-detection.
export async function saveCompilerPath(dir: string, path: string): Promise<void> {
  try {
    await invoke("save_compiler_path", { dir, path });
  } catch (error) {
    console.error(error);
  }
}

// Returns whether `dir`'s project runs PapyrusCompiler.exe against a
// dropped .psc as part of linting it. Returns false if the lookup fails.
export async function loadCompileCheck(dir: string): Promise<boolean> {
  try {
    return await invoke<boolean>("load_compile_check", { dir });
  } catch (error) {
    console.error(error);
    return false;
  }
}

// Persists whether `dir`'s project runs PapyrusCompiler.exe against a
// dropped .psc as part of linting it.
export async function saveCompileCheck(dir: string, enabled: boolean): Promise<void> {
  try {
    await invoke("save_compile_check", { dir, enabled });
  } catch (error) {
    console.error(error);
  }
}

// Returns `dir`'s configured additional script root directories, if any.
// Returns an empty array if none are configured or the lookup fails.
export async function loadScriptRoots(dir: string): Promise<string[]> {
  try {
    return await invoke<string[]>("load_script_roots", { dir });
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Returns `dir`'s configured analysis-only lookup directories, if any.
// Returns an empty array if none are configured or the lookup fails.
export async function loadLookupScriptRoots(dir: string): Promise<string[]> {
  try {
    return await invoke<string[]>("load_lookup_script_roots", { dir });
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Persists `roots` as `dir`'s configured additional script root
// directories.
export async function saveScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_script_roots", { dir, roots });
  } catch (error) {
    console.error(error);
  }
}

// Persists `roots` as `dir`'s configured analysis-only lookup directories.
export async function saveLookupScriptRoots(dir: string, roots: string[]): Promise<void> {
  try {
    await invoke("save_lookup_script_roots", { dir, roots });
  } catch (error) {
    console.error(error);
  }
}

// The project root (see projectDirForAchlist/projectDirForPscPath), also
// used by the "Argument type check" lint to resolve calls to functions
// declared on other scripts under it.
export let currentProjectDir: string | null = null;
// The PapyrusCompiler.exe path to use for the "Compile" button, kept in
// sync with the Settings tab's input (see handleCompilerPathChanged).
export let currentCompilerPath = "";
// Whether linting also runs PapyrusCompiler.exe against a dropped .psc,
// kept in sync with the Settings tab's checkbox (see
// handleCompileCheckChanged).
export let currentCompileCheck = false;
// Extra directories (besides scripts/source and source/scripts under the
// project root) to search for .psc files when resolving cross-script
// lookups, kept in sync with the Settings tab's textarea (see
// handleScriptRootsChanged).
export let currentScriptRoots: string[] = [];
// Extra directories searched only as a last-resort fallback when resolving
// a script by name for analysis (cross-script type/function lookups,
// Extends, autocompletion). Scripts found only here are never linted, and
// these directories are never considered by conflicting-script-versions.
// Kept in sync with the Settings tab's "Lookup script roots" textarea.
export let currentLookupScriptRoots: string[] = [];
// Source directories inferred from the entries in the currently loaded
// achlist. These are runtime-only roots: unlike currentScriptRoots, they are
// not displayed as user configuration or persisted to papyrus-lint.yaml.
export let currentAchlistScriptRoots: string[] = [];
// A dropped .ppj's own <Import> search paths (see parse_ppj_file), mirroring
// the CLI's ppj_imports (papyrus_lint_cli::run_scan::collect_script_paths):
// the project's additional_script_roots equivalent. Runtime-only, like
// currentAchlistScriptRoots above.
export let currentPpjImportRoots: string[] = [];

export function setCurrentProjectDir(dir: string | null) {
  currentProjectDir = dir;
}

export function setCurrentCompilerPath(path: string) {
  currentCompilerPath = path;
}

export function setCurrentCompileCheck(enabled: boolean) {
  currentCompileCheck = enabled;
}

export function setCurrentScriptRoots(roots: string[]) {
  currentScriptRoots = roots;
}

export function setCurrentLookupScriptRoots(roots: string[]) {
  currentLookupScriptRoots = roots;
}

export function effectiveScriptRoots(): string[] {
  return [
    ...new Set([...currentScriptRoots, ...currentAchlistScriptRoots, ...currentPpjImportRoots]),
  ];
}

// Sets the runtime-only script roots inferred from the currently loaded
// achlist (see currentAchlistScriptRoots above). Called by handleDroppedPaths
// as part of loading a drop, since that state belongs to the project module.
export function setAchlistScriptRoots(roots: string[]) {
  currentAchlistScriptRoots = roots;
}

// Sets the runtime-only script roots from a dropped .ppj's own <Import>
// entries (see currentPpjImportRoots above). Called by handleDroppedPaths the
// same way setAchlistScriptRoots is, and reset to empty whenever a
// non-.ppj input is dropped.
export function setPpjImportRoots(roots: string[]) {
  currentPpjImportRoots = roots;
}

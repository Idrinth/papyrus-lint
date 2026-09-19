export interface ProjectInfo {
  detected_script_roots: string[];
  used_configuration_file: string | null;
}

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

export let configPathOverrideEl: HTMLInputElement | null = null;
export let compilerPathEl: HTMLInputElement | null = null;
export let compileCheckEl: HTMLInputElement | null = null;
export let scriptRootsEl: HTMLTextAreaElement | null = null;
export let lookupScriptRootsEl: HTMLTextAreaElement | null = null;
export let detectedScriptRootsEl: HTMLOutputElement | null = null;
export let usedConfigurationFileEl: HTMLOutputElement | null = null;
export let settingsFieldsetEl: HTMLFieldSetElement | null = null;
export let settingsLockedNoticeEl: HTMLElement | null = null;

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
  return [...new Set([...currentScriptRoots, ...currentAchlistScriptRoots])];
}

// Sets the runtime-only script roots inferred from the currently loaded
// achlist (see currentAchlistScriptRoots above). Called by handleDroppedPaths
// as part of loading a drop, since that state belongs to the project module.
export function setAchlistScriptRoots(roots: string[]) {
  currentAchlistScriptRoots = roots;
}

// Reads the Settings tab's "Configuration file" override input, trimmed. An
// empty string means no override is set, so the lint config is auto-detected
// from the current project directory as usual.
export function configPathOverride(): string {
  return configPathOverrideEl?.value.trim() ?? "";
}

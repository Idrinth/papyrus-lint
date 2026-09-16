import { invoke } from "@tauri-apps/api/core";
import { loadAndApplyLintConfig } from "./config";
import { markLintResultsStale } from "./main";
import { applyConfigPreset, promptForConfigSelection } from "./presets";

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
let currentScriptRoots: string[] = [];
// Extra directories searched only as a last-resort fallback when resolving
// a script by name for analysis (cross-script type/function lookups,
// Extends, autocompletion). Scripts found only here are never linted, and
// these directories are never considered by conflicting-script-versions.
// Kept in sync with the Settings tab's "Lookup script roots" textarea.
export let currentLookupScriptRoots: string[] = [];
// Source directories inferred from the entries in the currently loaded
// achlist. These are runtime-only roots: unlike currentScriptRoots, they are
// not displayed as user configuration or persisted to papyrus-lint.yaml.
let currentAchlistScriptRoots: string[] = [];

let configPathOverrideEl: HTMLInputElement | null;
let compilerPathEl: HTMLInputElement | null;
let compileCheckEl: HTMLInputElement | null;
let scriptRootsEl: HTMLTextAreaElement | null;
let lookupScriptRootsEl: HTMLTextAreaElement | null;
let detectedScriptRootsEl: HTMLOutputElement | null;
let usedConfigurationFileEl: HTMLOutputElement | null;
let settingsFieldsetEl: HTMLFieldSetElement | null;
let settingsLockedNoticeEl: HTMLElement | null;

export function effectiveScriptRoots(): string[] {
  return [...new Set([...currentScriptRoots, ...currentAchlistScriptRoots])];
}

// Sets the runtime-only script roots inferred from the currently loaded
// achlist (see currentAchlistScriptRoots above). Called by handleDroppedPaths
// as part of loading a drop, since that state belongs to project.ts.
export function setAchlistScriptRoots(roots: string[]) {
  currentAchlistScriptRoots = roots;
}

export async function loadProjectInfo(dir: string): Promise<ProjectInfo> {
  try {
    return await invoke<ProjectInfo>("load_project_info", { dir });
  } catch (error) {
    console.error(error);
    return { detected_script_roots: [], used_configuration_file: null };
  }
}

export function applyProjectInfoToUI(info: ProjectInfo) {
  if (detectedScriptRootsEl) {
    detectedScriptRootsEl.textContent = info.detected_script_roots.length
      ? info.detected_script_roots.join("\n")
      : "None detected";
  }
  if (usedConfigurationFileEl) {
    usedConfigurationFileEl.textContent = info.used_configuration_file ?? "None (using defaults)";
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

// Splits the additional script roots textarea's value into one directory
// per non-blank line.
export function scriptRootsFromUI(): string[] {
  return (scriptRootsEl?.value ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

// Reflects `roots` onto the additional script roots textarea, one per line.
export function applyScriptRootsToUI(roots: string[]) {
  if (scriptRootsEl) {
    scriptRootsEl.value = roots.join("\n");
  }
}

// Called when the additional script roots textarea changes: updates the
// roots used to resolve cross-script lookups/the compiler's -i argument,
// and persists them to the current project's config file (if a project is
// loaded).
export function handleScriptRootsChanged() {
  currentScriptRoots = scriptRootsFromUI();
  markLintResultsStale();
  if (currentProjectDir) {
    void saveScriptRoots(currentProjectDir, currentScriptRoots);
  }
}

// Splits the lookup script roots textarea's value into one directory
// per non-blank line.
export function lookupScriptRootsFromUI(): string[] {
  return (lookupScriptRootsEl?.value ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

// Reflects `roots` onto the lookup script roots textarea, one per line.
export function applyLookupScriptRootsToUI(roots: string[]) {
  if (lookupScriptRootsEl) {
    lookupScriptRootsEl.value = roots.join("\n");
  }
}

// Called when the lookup script roots textarea changes: updates the
// analysis-only fallback directories used to resolve cross-script lookups,
// and persists them to the current project's config file (if a project is
// loaded). These are never mixed into additional script roots, so they are
// not linted and are ignored by conflicting-script-versions.
export function handleLookupScriptRootsChanged() {
  currentLookupScriptRoots = lookupScriptRootsFromUI();
  markLintResultsStale();
  if (currentProjectDir) {
    void saveLookupScriptRoots(currentProjectDir, currentLookupScriptRoots);
  }
}

// Called when the PapyrusCompiler.exe path input changes: updates the path
// used by the "Compile" button and persists it to the current project's
// config file (if a project is loaded).
export function handleCompilerPathChanged() {
  currentCompilerPath = compilerPathEl?.value ?? "";
  markLintResultsStale();
  if (currentProjectDir && compilerPathEl) {
    void saveCompilerPath(currentProjectDir, compilerPathEl.value);
  }
}

// Called when the "Also check with PapyrusCompiler.exe while linting"
// checkbox changes: updates whether lintPscFile/repairPscFile include
// compiler-reported errors, and persists the choice to the current
// project's config file (if a project is loaded).
export function handleCompileCheckChanged() {
  currentCompileCheck = compileCheckEl?.checked ?? false;
  markLintResultsStale();
  if (currentProjectDir) {
    void saveCompileCheck(currentProjectDir, currentCompileCheck);
  }
}

// Reads the Settings tab's "Configuration file" override input, trimmed. An
// empty string means no override is set, so the lint config is auto-detected
// from the current project directory as usual.
export function configPathOverride(): string {
  return configPathOverrideEl?.value.trim() ?? "";
}

// Locks (while `locked`) or unlocks the entire Settings tab. A project's
// configuration is picked per drop (see promptForConfigSelection/
// useProjectDir below), so until that pick is made for the
// currently-loading project, the Settings tab must not be shown/editable at
// all - otherwise it'd display (and let the user edit) the previous
// project's configuration, or the engine's silent defaults, before this
// drop's own configuration is even known, which doesn't make sense once
// more than one project is involved. The native <fieldset disabled>
// wrapping every Settings tab control (settingsFieldsetEl) handles
// keyboard/mouse interaction and accessibility on its own; the notice
// paragraph is a sibling of that fieldset (so it stays visible/announced
// while locked) explaining why the tab is inert.
export function setSettingsLocked(locked: boolean) {
  if (settingsFieldsetEl) {
    settingsFieldsetEl.disabled = locked;
  }
  if (settingsLockedNoticeEl) {
    settingsLockedNoticeEl.hidden = !locked;
  }
}

export async function useProjectDir(dir: string) {
  currentProjectDir = dir;
  const override = configPathOverride();
  const projectInfo = override ? null : await loadProjectInfo(dir);

  await loadAndApplyLintConfig(dir, override);
  currentCompilerPath = await loadCompilerPath(dir);
  if (compilerPathEl) {
    compilerPathEl.value = currentCompilerPath;
  }
  currentCompileCheck = await loadCompileCheck(dir);
  if (compileCheckEl) {
    compileCheckEl.checked = currentCompileCheck;
  }
  currentScriptRoots = await loadScriptRoots(dir);
  applyScriptRootsToUI(currentScriptRoots);
  currentLookupScriptRoots = await loadLookupScriptRoots(dir);
  applyLookupScriptRootsToUI(currentLookupScriptRoots);
  applyProjectInfoToUI(projectInfo ?? (await loadProjectInfo(dir)));
  if (override && usedConfigurationFileEl) {
    usedConfigurationFileEl.textContent = override;
  }
}

// Project directories already confirmed via promptForConfigSelection this
// session (see loadProjectConfig below), so re-linting the same project
// again (e.g. dropping the same achlist a second time) doesn't re-show the
// picker every time - only a directory not yet seen this session needs to
// go through it.
const confirmedProjectDirs = new Set<string>();

// Exposed for tests only: forgets every directory confirmed this session,
// so a test reusing the same directory string as an earlier one isn't
// short-circuited by that earlier test's confirmation.
export function resetConfirmedProjectDirs() {
  confirmedProjectDirs.clear();
}

// The entry point every real drop (handleDroppedPaths) calls instead of
// useProjectDir directly: it's what actually picks `dir`'s configuration
// (via promptForConfigSelection, unless `dir` was already confirmed this
// session) before handing off to useProjectDir to load and apply it,
// keeping the Settings tab locked for the whole of that pick (see
// setSettingsLocked) so it can never show/edit a configuration before one
// has actually been chosen for the project in play. useProjectDir itself
// stays reusable on its own (as plenty of tests do, and as the app's own
// startup restore of the last project directory does) for just loading an
// already-decided directory's configuration, without going through the
// picker at all - the picker is for a project the user is actively
// dropping into the app, not one merely remembered from a previous
// session.
export async function loadProjectConfig(dir: string): Promise<void> {
  if (!confirmedProjectDirs.has(dir)) {
    setSettingsLocked(true);
    const decision = await promptForConfigSelection(await loadProjectInfo(dir));
    if (decision.kind === "preset") {
      await applyConfigPreset(dir, decision.preset);
    }
    if (configPathOverrideEl) {
      configPathOverrideEl.value = decision.kind === "path" ? decision.path : "";
    }
    confirmedProjectDirs.add(dir);
  }
  await useProjectDir(dir);
  setSettingsLocked(false);
}

// Called when the "Configuration file" override input changes: reloads the
// current project's lint configuration from the new source (the override
// path, or back to auto-detection if it was cleared). Only reachable once
// the Settings tab is unlocked, i.e. after that project's configuration has
// already been picked via loadProjectConfig, so this never re-shows that
// picker - it's ordinary editing of an already-picked configuration.
export async function handleConfigPathOverrideChanged() {
  markLintResultsStale();
  if (currentProjectDir) {
    await useProjectDir(currentProjectDir);
    // useProjectDir may have replaced currentLintConfig (and the other
    // settings it reloads) after a relint already ran against the old
    // values, if the Lint results tab was clicked while this reload was
    // still in flight (relintCurrentFiles clears lintResultsStale as soon
    // as it starts, well before this await resolves). Re-marking it stale
    // here, unconditionally, is what makes the next tab switch re-lint
    // against the config this reload actually settled on, regardless of
    // whether that race happened.
    markLintResultsStale();
  }
}

export function bindProjectSettings() {
  configPathOverrideEl = document.querySelector("#config-path-override");
  compilerPathEl = document.querySelector("#compiler-path");
  compileCheckEl = document.querySelector("#compile-check");
  scriptRootsEl = document.querySelector("#script-roots");
  lookupScriptRootsEl = document.querySelector("#lookup-script-roots");
  detectedScriptRootsEl = document.querySelector("#detected-script-roots");
  usedConfigurationFileEl = document.querySelector("#used-configuration-file");
  settingsFieldsetEl = document.querySelector("#settings-fieldset");
  settingsLockedNoticeEl = document.querySelector("#settings-locked-notice");

  configPathOverrideEl?.addEventListener("change", handleConfigPathOverrideChanged);
  compilerPathEl?.addEventListener("change", handleCompilerPathChanged);
  compileCheckEl?.addEventListener("change", handleCompileCheckChanged);
  scriptRootsEl?.addEventListener("change", handleScriptRootsChanged);
  lookupScriptRootsEl?.addEventListener("change", handleLookupScriptRootsChanged);

  // No project's configuration is loaded yet at startup, so the Settings
  // tab starts locked (see setSettingsLocked); the markup itself also
  // starts with the wrapping fieldset disabled, so this just keeps the
  // notice paragraph in sync with it from the start. It stays locked until
  // the user actually drops something this session and loadProjectConfig
  // unlocks it - the app never restores a previous session's project.
  setSettingsLocked(true);
}

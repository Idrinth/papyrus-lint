import { loadAndApplyLintConfig } from "./config-ui";
import { markLintResultsStale } from "./drop";
import { applyConfigPreset } from "./presets-api";
import { promptForConfigSelection } from "./presets-picker";
import { loadCompileCheck, loadCompilerPath, loadLookupScriptRoots, loadProjectInfo, loadScriptRoots, saveCompileCheck, saveCompilerPath, saveLookupScriptRoots, saveScriptRoots } from "./project-io";
import { type ProjectInfo, bindProjectSettingsDom, compileCheckEl, compilerPathEl, configPathOverride, configPathOverrideEl, currentProjectDir, detectedScriptRootsEl, lookupScriptRootsEl, scriptRootsEl, setCurrentCompileCheck, setCurrentCompilerPath, setCurrentLookupScriptRoots, setCurrentProjectDir, setCurrentScriptRoots, settingsFieldsetEl, settingsLockedNoticeEl, usedConfigurationFileEl } from "./project-state";

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
  const roots = scriptRootsFromUI();
  setCurrentScriptRoots(roots);
  markLintResultsStale();
  if (currentProjectDir) {
    void saveScriptRoots(currentProjectDir, roots);
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
  const roots = lookupScriptRootsFromUI();
  setCurrentLookupScriptRoots(roots);
  markLintResultsStale();
  if (currentProjectDir) {
    void saveLookupScriptRoots(currentProjectDir, roots);
  }
}

// Called when the PapyrusCompiler.exe path input changes: updates the path
// used by the "Compile" button and persists it to the current project's
// config file (if a project is loaded).
export function handleCompilerPathChanged() {
  const path = compilerPathEl?.value ?? "";
  setCurrentCompilerPath(path);
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
  const enabled = compileCheckEl?.checked ?? false;
  setCurrentCompileCheck(enabled);
  markLintResultsStale();
  if (currentProjectDir) {
    void saveCompileCheck(currentProjectDir, enabled);
  }
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
  setCurrentProjectDir(dir);
  const override = configPathOverride();
  const projectInfo = override ? null : await loadProjectInfo(dir);

  await loadAndApplyLintConfig(dir, override);
  const compilerPath = await loadCompilerPath(dir);
  setCurrentCompilerPath(compilerPath);
  if (compilerPathEl) {
    compilerPathEl.value = compilerPath;
  }
  const compileCheck = await loadCompileCheck(dir);
  setCurrentCompileCheck(compileCheck);
  if (compileCheckEl) {
    compileCheckEl.checked = compileCheck;
  }
  const scriptRoots = await loadScriptRoots(dir);
  setCurrentScriptRoots(scriptRoots);
  applyScriptRootsToUI(scriptRoots);
  const lookupRoots = await loadLookupScriptRoots(dir);
  setCurrentLookupScriptRoots(lookupRoots);
  applyLookupScriptRootsToUI(lookupRoots);
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

// Forget every directory confirmed this session, so a test reusing the same
// directory string as an earlier one isn't short-circuited by that earlier
// test's confirmation.
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
  bindProjectSettingsDom();

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

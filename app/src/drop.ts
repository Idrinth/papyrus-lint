// Drop-folder handling: turning a dropped .achlist/.ppj/.psc/directory into a
// project root and a parsed+linted set of results, and re-linting that same
// set later against changed settings. Kept separate from the page-chrome
// orchestration in main.ts, which this module calls back into to render its
// results.
import { invoke } from "@tauri-apps/api/core";
import { lintPscFile, type PapyrusScript, type PscParseOutcome } from "./backend";
import { clearError, setDropZoneLoading, showError, showResult } from "./main";
import { switchTab } from "./main-tabs";
import { isAchlistPath, isPpjPath, isPscPath, scriptRootsForAchlist } from "./path";
import { scheduleHideLintProgress, showLintProgress, updateLintProgress } from "./progress";
import { loadProjectConfig } from "./project-settings";
import {
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPpj,
  projectDirForPscPath,
} from "./project-io";
import { setAchlistScriptRoots, setPpjImportRoots } from "./project-state";
import { renderPscResults } from "./results-list-render";
export let currentPscOutcomes: PscParseOutcome[] = [];

// Mirrors the backend's PpjParseResult (see parse_ppj_file in
// app/src-tauri/src/files.rs): a dropped .ppj's own .psc entries and
// <Import> search paths.
interface PpjParseResult {
  scripts: string[];
  imports: string[];
}

// Set whenever a setting affecting lint output (formatting/rule config,
// compiler path, compile-check toggle, additional/lookup script roots, or the
// configuration file override) changes after currentPscOutcomes was last
// populated, so a currently showing lint results list no longer reflects
// the active settings. Checked by the Lint results tab button so switching
// to it re-lints the same files instead of silently showing stale findings.
export let lintResultsStale = false;

// Marks the currently shown lint results as no longer reflecting the active
// settings (see lintResultsStale above). Called by config.ts/project.ts
// whenever a setting affecting lint output changes, since drop/relint flow
// is otherwise the only thing that reads or clears it.
export function markLintResultsStale() {
  lintResultsStale = true;
}

// Bumped by handleDroppedPaths every time a new drop starts parsing/linting;
// a still-running drop's parsePscFiles callback checks its own snapshot of
// this against the current value before touching currentPscOutcomes, so a
// straggling outcome from a drop superseded by a newer one can't get mixed
// into the newer drop's results.
let currentParseGeneration = 0;

// Kept separate from currentParseGeneration because directory enumeration
// happens before parsing begins. A superseded, slower drop must not hide the
// newer drop's loading indicator when its own listing eventually completes.
let currentListingGeneration = 0;

// How many scripts parsePscFiles works on at once, mirroring the CLI's own
// --threads default (papyrus_lint_core::parallel::default_thread_count): the
// machine's hardware concurrency, so dropping a large achlist doesn't fire
// every one of its scripts' worth of Tauri commands at the same instant —
// each is dispatched to its own thread, and Rust's own parsing/linting work
// is CPU-bound, so far more in flight than the machine has cores to run them
// on just adds contention without finishing any of them sooner. Falls back
// to 4 when the runtime doesn't report hardwareConcurrency (or reports an
// implausible non-positive value).
function parseConcurrencyLimit(): number {
  const cores = typeof navigator === "object" && navigator ? navigator.hardwareConcurrency : 0;
  return cores && cores > 0 ? cores : 4;
}

// Runs `fn` over every item in `items`, at most `limit` calls in flight at
// once, resolving to their results in `items`' own order regardless of which
// order they actually finish in — the same ordering guarantee
// Promise.all(items.map(fn)) gives, but without starting every call at once.
async function mapWithConcurrency<T, R>(
  items: T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  const results: R[] = new Array(items.length);
  let nextIndex = 0;
  async function worker() {
    while (nextIndex < items.length) {
      const index = nextIndex++;
      results[index] = await fn(items[index], index);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, worker));
  return results;
}

// Parses and lints every path in `paths`, invoking `onOutcome` (if given) as
// each one finishes rather than waiting for the whole batch — the caller can
// use that to render results incrementally instead of freezing until the
// slowest file completes. Outcomes are otherwise still resolved concurrently
// (up to parseConcurrencyLimit() at once), so `onOutcome` fires in
// completion order, not necessarily `paths`' own order.
export async function parsePscFiles(
  paths: string[],
  onOutcome?: (outcome: PscParseOutcome) => void,
): Promise<PscParseOutcome[]> {
  return mapWithConcurrency(paths, parseConcurrencyLimit(), async (path) => {
    let outcome: PscParseOutcome;
    try {
      const script = await invoke<PapyrusScript>("parse_psc_file", { path });
      const findings = await lintPscFile(path);
      outcome = { path, ok: true, detail: `parsed as "${script.name}"`, findings };
    } catch (error) {
      outcome = { path, ok: false, detail: String(error), findings: [] };
    }
    onOutcome?.(outcome);
    return outcome;
  });
}

export async function handleDroppedPaths(paths: string[]) {
  const listingGeneration = ++currentListingGeneration;
  setDropZoneLoading(true);
  const finishListing = () => {
    if (listingGeneration === currentListingGeneration) {
      setDropZoneLoading(false);
    }
  };
  const achlistPath = paths.find(isAchlistPath);

  if (achlistPath) {
    try {
      const entries = await invoke<string[]>("parse_achlist_file", {
        path: achlistPath,
      });
      clearError();
      // Cleared before rendering so a View click during the parse/lint
      // pass below can't show a previous drop's stale findings for a
      // path that happens to match one of this drop's entries.
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = await projectDirForAchlist(achlistPath, entries);
      showResult(achlistPath, entries, projectDir);
      finishListing();
      switchTab("lint");
      renderPscResults(currentPscOutcomes);

      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
      setPpjImportRoots([]);
      const pscEntries = entries.filter(isPscPath);
      showLintProgress(pscEntries.length);
      await parsePscFiles(pscEntries, (outcome) => {
        // A newer drop started (and so already reset currentPscOutcomes to
        // its own array) while this one was still parsing/linting; don't
        // mix this stale outcome into it.
        if (generation !== currentParseGeneration) {
          return;
        }
        currentPscOutcomes.push(outcome);
        renderPscResults(currentPscOutcomes);
        updateLintProgress(currentPscOutcomes.length, pscEntries.length);
      });
      if (generation === currentParseGeneration) {
        scheduleHideLintProgress();
      }
    } catch (error) {
      finishListing();
      showError("Failed to read that .achlist file. Please try again.");
      console.error(error);
    }
    return;
  }

  const ppjPath = paths.find(isPpjPath);

  if (ppjPath) {
    try {
      const { scripts, imports } = await invoke<PpjParseResult>("parse_ppj_file", {
        path: ppjPath,
      });
      clearError();
      // Cleared before rendering, same as the .achlist branch above, so a
      // View click during the parse/lint pass below can't show a previous
      // drop's stale findings for a path that happens to match one of this
      // drop's scripts.
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = await projectDirForPpj(ppjPath, scripts);
      showResult(ppjPath, scripts, projectDir);
      finishListing();
      switchTab("lint");
      renderPscResults(currentPscOutcomes);

      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(scripts));
      setPpjImportRoots(imports);
      showLintProgress(scripts.length);
      await parsePscFiles(scripts, (outcome) => {
        if (generation !== currentParseGeneration) {
          return;
        }
        currentPscOutcomes.push(outcome);
        renderPscResults(currentPscOutcomes);
        updateLintProgress(currentPscOutcomes.length, scripts.length);
      });
      if (generation === currentParseGeneration) {
        scheduleHideLintProgress();
      }
    } catch (error) {
      finishListing();
      showError("Failed to read that .ppj file. Please try again.");
      console.error(error);
    }
    return;
  }

  if (paths.length === 1 && isPscPath(paths[0])) {
    const pscPath = paths[0];
    clearError();
    currentPscOutcomes = [];
    lintResultsStale = false;
    const generation = ++currentParseGeneration;
    const projectDir = await projectDirForPscPath(pscPath);
    showResult(pscPath, [pscPath], projectDir);
    finishListing();
    switchTab("lint");
    renderPscResults(currentPscOutcomes);

    await loadProjectConfig(projectDir);
    setAchlistScriptRoots([]);
    setPpjImportRoots([]);
    showLintProgress(1);
    await parsePscFiles([pscPath], (outcome) => {
      if (generation !== currentParseGeneration) {
        return;
      }
      currentPscOutcomes.push(outcome);
      renderPscResults(currentPscOutcomes);
      updateLintProgress(currentPscOutcomes.length, 1);
    });
    if (generation === currentParseGeneration) {
      scheduleHideLintProgress();
    }
    return;
  }

  // Neither an .achlist, a .ppj, nor a single .psc: try treating the single
  // dropped path as a directory to scan recursively for .psc files, for a
  // project (e.g. Requiem's own layout) with no .achlist at all whose
  // scripts are spread across arbitrarily nested subfolders.
  // list_psc_files_recursively errors out if the path isn't actually a
  // directory, so that case falls through to the usual error message below.
  if (paths.length === 1) {
    const dirPath = paths[0];
    try {
      const entries = await invoke<string[]>("list_psc_files_recursively", {
        path: dirPath,
      });
      clearError();
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = await projectDirForDirectory(dirPath, entries);
      showResult(dirPath, entries, projectDir);
      finishListing();
      switchTab("lint");
      renderPscResults(currentPscOutcomes);

      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
      setPpjImportRoots([]);
      showLintProgress(entries.length);
      await parsePscFiles(entries, (outcome) => {
        if (generation !== currentParseGeneration) {
          return;
        }
        currentPscOutcomes.push(outcome);
        renderPscResults(currentPscOutcomes);
        updateLintProgress(currentPscOutcomes.length, entries.length);
      });
      if (generation === currentParseGeneration) {
        scheduleHideLintProgress();
      }
      return;
    } catch {
      // Not a directory either; fall through to the error below.
    }
  }

  finishListing();
  showError("Please drop a single .achlist, .ppj, or .psc file, or a folder to scan recursively.");
}

// Re-lints the same set of files currently shown in the Lint results tab
// against the now-current settings. Called when that tab is switched to
// while lintResultsStale is set, so a settings change (formatting/rule
// config, compiler path, compile-check toggle, script roots, or the
// configuration file override) takes visible effect instead of leaving
// stale findings on screen. Clears the list first (rather than relinting
// in place) so it's obvious a fresh pass is running rather than silently
// showing results that may no longer match the current settings.
export async function relintCurrentFiles() {
  const paths = currentPscOutcomes.map((outcome) => outcome.path);
  if (paths.length === 0) {
    return;
  }
  lintResultsStale = false;
  currentPscOutcomes = [];
  const generation = ++currentParseGeneration;
  switchTab("lint");
  renderPscResults(currentPscOutcomes);

  showLintProgress(paths.length);
  await parsePscFiles(paths, (outcome) => {
    if (generation !== currentParseGeneration) {
      return;
    }
    currentPscOutcomes.push(outcome);
    renderPscResults(currentPscOutcomes);
    updateLintProgress(currentPscOutcomes.length, paths.length);
  });
  if (generation === currentParseGeneration) {
    scheduleHideLintProgress();
  }
}

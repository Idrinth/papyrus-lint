// Drop-folder handling: turning a dropped .achlist/.ppj/.psc/directory into a
// project root and a parsed+linted set of results, and re-linting that same
// set later against changed settings. Kept separate from the page-chrome
// orchestration in main.ts, which this module calls back into to render its
// results.
import { invoke } from "@tauri-apps/api/core";
import { type PapyrusScript, type PscParseOutcome } from "./backend-types";
import { type ProjectLintEvent, lintPscFile, lintProjectScripts } from "./backend";
import { currentLintConfig } from "./config-types";
import { clearError, setDropZoneLoading, showError, showResult } from "./main";
import { switchTab } from "./main-tabs";
import { isAchlistPath, isPpjPath, isPscPath, scriptRootsForAchlist } from "./path";
import { scheduleHideLintProgress, showLintActivity, updateLintProgress } from "./progress";
import { loadProjectConfig } from "./project-settings";
import {
  projectDirForAchlist,
  projectDirForDirectory,
  projectDirForPpj,
  projectDirForPscPath,
} from "./project-io";
import { setAchlistScriptRoots, setCurrentProjectScripts, setPpjImportRoots } from "./project-state";
import { appendStreamedPscResult, renderPscResults } from "./results-list-render";
export let currentPscOutcomes: PscParseOutcome[] = [];

interface PpjParseResult {
  scripts: string[];
  imports: string[];
}

export let lintResultsStale = false;

export function markLintResultsStale() {
  lintResultsStale = true;
}

let currentParseGeneration = 0;
let currentListingGeneration = 0;

function parseConcurrencyLimit(): number {
  const cores = typeof navigator === "object" && navigator ? navigator.hardwareConcurrency : 0;
  return cores && cores > 0 ? cores : 4;
}

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

export async function parsePscFiles(
  paths: string[],
  onOutcome?: (outcome: PscParseOutcome) => void,
): Promise<PscParseOutcome[]> {
  return mapWithConcurrency(paths, parseConcurrencyLimit(), async (path) => {
    let outcome: PscParseOutcome;
    try {
      const script = await invoke<PapyrusScript>("parse_psc_file", { path, game: currentLintConfig.game });
      const findings = await lintPscFile(path);
      outcome = { path, ok: true, detail: `parsed as "${script.name}"`, findings };
    } catch (error) {
      outcome = { path, ok: false, detail: String(error), findings: [] };
    }
    onOutcome?.(outcome);
    return outcome;
  });
}

function applyProjectLintEvent(event: ProjectLintEvent) {
  if (event.kind === "result") {
    const outcome = {
      path: event.path,
      ok: event.ok,
      detail: event.detail,
      findings: event.findings,
    };
    currentPscOutcomes.push(outcome);
    // Append this file only. Rebuilding the whole list (and every finding
    // row) as each file finished was quadratic in the batch size.
    appendStreamedPscResult(currentPscOutcomes, outcome);
    return;
  }
  if (event.total <= 0) {
    showLintActivity(event.phase);
    return;
  }
  // Same contract as the CLI's "Parsing: n/total" line, then "Linting":
  // one bar, and `total` grows when a referenced script is pushed onto
  // the parse queue.
  updateLintProgress(event.completed, event.total, event.phase);
}

async function runParseThenLint(paths: string[], generation: number) {
  // Don't start on a determinate "Parsing 0 / N" bar. The command still has
  // to ship the path list across IPC, build the function table, and preload
  // the collision cache before the first parse callback fires — on a 5k-file
  // drop that gap looks like a hung bar. An indeterminate phase names the
  // wait; the first `total > 0` event from the command turns it into a bar.
  showLintActivity(preparingFilesLabel(paths.length));
  await lintProjectScripts(
    paths,
    paths.length === 0
      ? undefined
      : (event) => {
          if (generation !== currentParseGeneration) {
            return;
          }
          applyProjectLintEvent(event);
        },
  );
  if (generation === currentParseGeneration) {
    scheduleHideLintProgress();
  }
}

function preparingFilesLabel(count: number): string {
  if (count === 1) {
    return "Preparing 1 file";
  }
  return `Preparing ${count} files`;
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
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = await projectDirForAchlist(achlistPath, entries);
      showResult(achlistPath, entries, projectDir);
      finishListing();
      switchTab("lint");
      renderPscResults(currentPscOutcomes);

      showLintActivity("Loading project settings");
      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
      setPpjImportRoots([]);
      const pscEntries = entries.filter(isPscPath);
      setCurrentProjectScripts(pscEntries);
      await runParseThenLint(pscEntries, generation);
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
      if (listingGeneration !== currentListingGeneration) {
        return;
      }
      clearError();
      currentPscOutcomes = [];
      lintResultsStale = false;
      const generation = ++currentParseGeneration;
      const projectDir = await projectDirForPpj(ppjPath, scripts);
      if (listingGeneration !== currentListingGeneration) {
        return;
      }
      showResult(ppjPath, scripts, projectDir);
      finishListing();
      switchTab("lint");
      renderPscResults(currentPscOutcomes);

      showLintActivity("Loading project settings");
      await loadProjectConfig(projectDir);
      if (listingGeneration !== currentListingGeneration) {
        return;
      }
      setAchlistScriptRoots(scriptRootsForAchlist(scripts));
      setPpjImportRoots(imports);
      setCurrentProjectScripts(scripts);
      await runParseThenLint(scripts, generation);
    } catch (error) {
      finishListing();
      if (listingGeneration !== currentListingGeneration) {
        return;
      }
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

    showLintActivity("Loading project settings");
    await loadProjectConfig(projectDir);
    setAchlistScriptRoots([]);
    setPpjImportRoots([]);
    setCurrentProjectScripts([pscPath]);
    await runParseThenLint([pscPath], generation);
    return;
  }

  if (paths.length === 1) {
    const dirPath = paths[0];
    try {
      showLintActivity("Listing scripts");
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

      showLintActivity("Loading project settings");
      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
      setPpjImportRoots([]);
      setCurrentProjectScripts(entries);
      await runParseThenLint(entries, generation);
      return;
    } catch {
      // Not a directory either; fall through to the error below.
    }
  }

  finishListing();
  showError("Please drop a single .achlist, .ppj, or .psc file, or a folder to scan recursively.");
}

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

  await runParseThenLint(paths, generation);
}

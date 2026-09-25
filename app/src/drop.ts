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
import { scheduleHideLintProgress, showLintActivity, showLintProgress, updateLintProgress } from "./progress";
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
    currentPscOutcomes.push({
      path: event.path,
      ok: event.ok,
      detail: event.detail,
      findings: event.findings,
    });
    renderPscResults(currentPscOutcomes);
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
  // One in-process batch: parse the type closure (the bar starts at the
  // lint-target count and grows as referenced scripts are enqueued), index
  // the function table, then lint. Results stream in completion order.
  showLintProgress(paths.length, "Parsing");
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

      await loadProjectConfig(projectDir);
      setAchlistScriptRoots(scriptRootsForAchlist(entries));
      setPpjImportRoots([]);
      const pscEntries = entries.filter(isPscPath);
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

      await loadProjectConfig(projectDir);
      if (listingGeneration !== currentListingGeneration) {
        return;
      }
      setAchlistScriptRoots(scriptRootsForAchlist(scripts));
      setPpjImportRoots(imports);
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

    await loadProjectConfig(projectDir);
    setAchlistScriptRoots([]);
    setPpjImportRoots([]);
    await runParseThenLint([pscPath], generation);
    return;
  }

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

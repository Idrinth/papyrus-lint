// Watch mode: while enabled, periodically polls the currently loaded .psc
// files' own last-modified times and automatically re-lints any that
// changed on disk since the previous poll — so an edit made outside the
// app (a text editor, a Creation Kit export) shows up in the Lint results
// tab without the user having to drop the project again. Polling (rather
// than an OS-level filesystem watcher) needs no extra native dependency and,
// since it just re-reads each watched file's current mtime, notices a
// change no matter how it was made, including the atomic
// write-to-a-temp-file-then-rename pattern many editors use for a "save",
// which a naive filesystem-event watcher can miss or double-report.
import { getPscFileMtimes } from "./backend";
import { currentPscOutcomes, parsePscFiles } from "./drop";
import { renderPscResults } from "./results-list";

// How often watch mode polls the backend for the watched files' mtimes.
const WATCH_POLL_INTERVAL_MS = 1500;

let watchTimerId: ReturnType<typeof window.setInterval> | null = null;
// The path list, and each of its paths' last-seen mtime, as of the previous
// poll — compared against on the next one to tell which files actually
// changed. Reset (rather than diffed) whenever the tracked path list itself
// changes (a new drop, or a manual re-lint), so that doesn't get reported as
// every file having changed.
let watchedPaths: string[] = [];
let watchBaseline: Map<string, number | undefined> = new Map();
// Guards against a slow poll still running when its own interval fires
// again, so two overlapping polls can't both try to re-lint the same
// changed file at once.
let watchPollInFlight = false;

let watchToggleEl: HTMLInputElement | null;
let watchStatusEl: HTMLElement | null;

export function isWatchModeEnabled(): boolean {
  return watchTimerId !== null;
}

function setWatchStatus(text: string) {
  if (watchStatusEl) {
    watchStatusEl.textContent = text;
  }
}

function fileCountLabel(count: number): string {
  return `${count} file${count === 1 ? "" : "s"}`;
}

// Re-lints every path in `paths` and splices each result back into
// currentPscOutcomes in place (matching persistCodeViewerEdits' own
// find-by-path update in live-edit.ts), then re-renders the results list.
async function relintChangedFiles(paths: string[]): Promise<void> {
  await parsePscFiles(paths, (outcome) => {
    const index = currentPscOutcomes.findIndex((existing) => existing.path === outcome.path);
    if (index !== -1) {
      currentPscOutcomes[index] = outcome;
    }
  });
  renderPscResults(currentPscOutcomes);
}

// One poll cycle: fetches the watched files' current mtimes and re-lints
// whichever ones changed (or newly appeared/disappeared) since the last
// poll.
async function pollWatchedFiles(): Promise<void> {
  if (watchPollInFlight) {
    return;
  }
  const paths = currentPscOutcomes.map((outcome) => outcome.path);
  if (paths.length === 0) {
    watchedPaths = [];
    watchBaseline = new Map();
    setWatchStatus("Watching: no files loaded yet");
    return;
  }

  watchPollInFlight = true;
  try {
    const mtimes = await getPscFileMtimes(paths);
    const currentBaseline = new Map<string, number | undefined>(paths.map((path) => [path, mtimes[path]]));
    const pathsChanged =
      paths.length !== watchedPaths.length || paths.some((path, index) => path !== watchedPaths[index]);

    if (pathsChanged) {
      // The tracked file set itself changed (a new drop or a manual
      // re-lint finished) since the previous poll; take a fresh baseline
      // rather than treating every file as changed just because the set
      // did.
      watchedPaths = paths;
      watchBaseline = currentBaseline;
      setWatchStatus(`Watching ${fileCountLabel(paths.length)} for changes`);
      return;
    }

    const changed = paths.filter((path) => watchBaseline.get(path) !== currentBaseline.get(path));
    watchBaseline = currentBaseline;
    if (changed.length === 0) {
      return;
    }

    setWatchStatus(`Re-linting ${fileCountLabel(changed.length)} that changed on disk…`);
    await relintChangedFiles(changed);
    setWatchStatus(`Watching ${fileCountLabel(paths.length)} for changes`);
  } finally {
    watchPollInFlight = false;
  }
}

// Starts watch mode: takes an immediate baseline of the currently loaded
// files (so the very first poll doesn't treat them all as "changed"), then
// polls every WATCH_POLL_INTERVAL_MS. A no-op if already running.
export function startWatchMode(): void {
  if (watchTimerId !== null) {
    return;
  }
  watchedPaths = [];
  watchBaseline = new Map();
  void pollWatchedFiles();
  watchTimerId = window.setInterval(() => void pollWatchedFiles(), WATCH_POLL_INTERVAL_MS);
}

// Stops watch mode. Safe to call whether or not it's currently running —
// used both by the toggle checkbox and to reset state between tests.
export function stopWatchMode(): void {
  if (watchTimerId !== null) {
    window.clearInterval(watchTimerId);
    watchTimerId = null;
  }
  setWatchStatus("");
}

function handleWatchToggleChange(): void {
  if (watchToggleEl?.checked) {
    startWatchMode();
  } else {
    stopWatchMode();
  }
}

export function bindWatchMode(): void {
  watchToggleEl = document.querySelector("#watch-mode-toggle");
  watchStatusEl = document.querySelector("#watch-mode-status");
  watchToggleEl?.addEventListener("change", handleWatchToggleChange);
}

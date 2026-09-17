import { afterEach, beforeEach, expect, vi } from "vitest";
import { invokeMock } from "./mocks";
import { mountFixture } from "./fixture";
import { cancelLiveEditLint, loadProjectConfig, resetConfirmedProjectDirs } from "../main";
import { dirnameOf } from "../path";

// Default backend behavior for the project-root discovery commands (see
// project.ts's projectDirForAchlist/projectDirForDirectory/
// projectDirForPscPath), for tests that drive handleDroppedPaths without
// caring about the exact root a particular drop resolves to: just the
// naive fallback each of those functions itself would use if the real
// (Rust) `scripts/source`/`source/scripts`-pair lookup found nothing. A
// test asserting a specific resolved root (e.g. one where the achlist
// doesn't live in the project root itself) still needs its own explicit
// `find_project_root`/`find_psc_project_root_for_path` handler.
function defaultProjectRootHandler(command: string): ((args: unknown) => unknown) | undefined {
  switch (command) {
    case "load_lookup_script_roots":
      return () => [];
    case "find_project_root":
      return (args) => (args as { fallback: string }).fallback;
    case "find_psc_project_root_for_path":
      return (args) => dirnameOf(dirnameOf(dirnameOf((args as { path: string }).path)));
    default:
      return undefined;
  }
}

export function invokeImplFor(handlers: Record<string, (args: unknown) => unknown>) {
  invokeMock.mockImplementation((command: string, args: unknown) => {
    const handler = handlers[command] ?? defaultProjectRootHandler(command);
    if (!handler) {
      return Promise.reject(new Error(`unexpected command: ${command}`));
    }
    return Promise.resolve(handler(args));
  });
}

// Waits for the "select this project's configuration" dialog
// (promptForConfigSelection) to open and clicks "Continue", accepting
// useProjectDir's own auto-detection either way (an existing configuration
// file, or the engine's silent defaults if the project has none). Pumps
// microtasks directly instead of vi.waitFor, so it works the same whether
// or not a test has switched to fake timers.
export async function confirmDetectedConfig(): Promise<void> {
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  expect(picker.hasAttribute("open")).toBe(true);
  document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
}

// Drives useProjectDir through loadProjectConfig's own "select this
// project's configuration" step for tests that don't care about that step
// itself, immediately accepting whatever useProjectDir would already do on
// its own (see confirmDetectedConfig). A directory already confirmed this
// session (see resetConfirmedProjectDirs) skips the dialog entirely, the
// same as loadProjectConfig itself does.
export async function loadProjectConfigConfirmed(dir: string): Promise<void> {
  const pending = loadProjectConfig(dir);
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  if (picker.hasAttribute("open")) {
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
  }
  await pending;
}

beforeEach(() => {
  invokeMock.mockReset();
  localStorage.clear();
  mountFixture();
  resetConfirmedProjectDirs();
  // A previous test's live-lint debounce timer (see scheduleLiveEditLint in
  // live-edit.ts) would otherwise fire against this test's fresh DOM/mocks once
  // its delay elapses, so it's cancelled up front the same way
  // resetConfirmedProjectDirs above resets other module-level state.
  cancelLiveEditLint();
});

afterEach(() => {
  vi.restoreAllMocks();
  cancelLiveEditLint();
});

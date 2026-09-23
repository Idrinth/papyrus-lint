import { describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ show: showWindowMock }),
}));

import { invokeImplFor } from "./test/harness";
import { handleDroppedPaths } from "./drop";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { isWatchModeEnabled, startWatchMode, stopWatchMode } from "./watch";
// Drops a single .psc file, confirming the "select this project's
// configuration" dialog if one comes up — it won't for a project directory
// already confirmed earlier in the same test (see loadProjectConfig in
// project-settings.ts), the way dropping a second file from the same project root
// does here.
async function dropSingleFile(path: string, mtime: number | undefined): Promise<void> {
  invokeImplFor({
    load_lint_config: () => DEFAULT_LINT_CONFIG,
    load_compiler_path: () => null,
    load_compile_check: () => false,
    load_script_roots: () => [],
    parse_psc_file: () => ({ name: "Example" }),
    lint_psc_file: () => [],
    get_psc_file_mtimes: () => (mtime === undefined ? {} : { [path]: mtime }),
  });
  const pending = handleDroppedPaths([path]);
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  if (picker.hasAttribute("open")) {
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
  }
  await pending;
}

function watchToggle(): HTMLInputElement {
  return document.querySelector<HTMLInputElement>("#watch-mode-toggle")!;
}

function watchStatus(): string {
  return document.querySelector<HTMLElement>("#watch-mode-status")!.textContent ?? "";
}

describe("watch mode", () => {
  it("is off until the toggle is checked, and stopWatchMode turns it back off", async () => {
    await dropSingleFile("/proj/scripts/source/A.psc", 1000);

    expect(isWatchModeEnabled()).toBe(false);

    watchToggle().checked = true;
    watchToggle().dispatchEvent(new Event("change"));
    expect(isWatchModeEnabled()).toBe(true);

    stopWatchMode();
    expect(isWatchModeEnabled()).toBe(false);
  });

  it("re-lints a watched file once its mtime changes on a later poll", async () => {
    vi.useFakeTimers();
    const path = "/proj/scripts/source/A.psc";
    await dropSingleFile(path, 1000);

    startWatchMode();
    // The very first poll (fired synchronously by startWatchMode) only
    // takes a baseline; let it settle before advancing the clock.
    await vi.advanceTimersByTimeAsync(0);
    expect(watchStatus()).toBe("Watching 1 file for changes");

    invokeImplFor({
      get_psc_file_mtimes: () => ({ [path]: 2000 }),
      parse_psc_file: () => ({ name: "Example" }),
      lint_psc_file: () => [{ line: 1, column: 1, message: "[warning] changed on disk" }],
    });

    await vi.advanceTimersByTimeAsync(1500);

    expect(invokeMock).toHaveBeenCalledWith("lint_psc_file", expect.objectContaining({ path }));
    expect(document.querySelectorAll("#psc-result-list .psc-result__finding")).toHaveLength(1);
    expect(watchStatus()).toBe("Watching 1 file for changes");

    vi.useRealTimers();
  });

  it("does not re-lint when the watched file's mtime is unchanged", async () => {
    vi.useFakeTimers();
    const path = "/proj/scripts/source/A.psc";
    await dropSingleFile(path, 1000);

    startWatchMode();
    await vi.advanceTimersByTimeAsync(0);
    invokeMock.mockClear();

    invokeImplFor({ get_psc_file_mtimes: () => ({ [path]: 1000 }) });
    await vi.advanceTimersByTimeAsync(1500);

    expect(invokeMock).not.toHaveBeenCalledWith("lint_psc_file", expect.anything());

    vi.useRealTimers();
  });

  it("re-baselines instead of re-linting everything when a new drop changes the watched file set", async () => {
    vi.useFakeTimers();
    await dropSingleFile("/proj/scripts/source/A.psc", 1000);

    startWatchMode();
    await vi.advanceTimersByTimeAsync(0);

    await dropSingleFile("/proj/scripts/source/B.psc", 5000);
    invokeMock.mockClear();
    invokeImplFor({ get_psc_file_mtimes: () => ({ "/proj/scripts/source/B.psc": 5000 }) });

    await vi.advanceTimersByTimeAsync(1500);

    expect(invokeMock).not.toHaveBeenCalledWith("lint_psc_file", expect.anything());
    expect(watchStatus()).toBe("Watching 1 file for changes");

    vi.useRealTimers();
  });
});

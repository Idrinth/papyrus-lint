import { describe, expect, it, vi } from "vitest";
import { invokeMock, onDragDropEventMock } from "./test/mocks";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

import "./test/harness";
import { hideLintProgress, scheduleHideLintProgress, showLintProgress, updateLintProgress } from "./progress";

describe("showLintProgress / updateLintProgress / hideLintProgress", () => {
  it("shows the progress bar reset to 0/total", () => {
    showLintProgress(3);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(0);
    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.max).toBe(3);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 0 / 3 files");
  });

  it("stays hidden when there are no files to process", () => {
    showLintProgress(0);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
  });

  it("updates the bar's value and label as files finish", () => {
    showLintProgress(2);
    updateLintProgress(1, 2);

    expect(document.querySelector<HTMLProgressElement>("#lint-progress-bar")!.value).toBe(1);
    expect(document.querySelector("#lint-progress-label")!.textContent).toBe("Linting 1 / 2 files");
  });

  it("hides the progress bar", () => {
    showLintProgress(2);
    hideLintProgress();

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
  });

  it("scheduleHideLintProgress keeps the bar visible during the grace period, then hides it", () => {
    vi.useFakeTimers();
    showLintProgress(2);
    updateLintProgress(2, 2);

    scheduleHideLintProgress();
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(1999);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(1);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
  });

  it("scheduleHideLintProgress's pending hide is cancelled by a new showLintProgress call", () => {
    vi.useFakeTimers();
    showLintProgress(2);
    updateLintProgress(2, 2);
    scheduleHideLintProgress();

    showLintProgress(3);
    vi.advanceTimersByTime(2000);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);
    vi.useRealTimers();
  });

  it("scheduleHideLintProgress replaces an already pending hide", () => {
    vi.useFakeTimers();
    showLintProgress(1);

    scheduleHideLintProgress(1000);
    vi.advanceTimersByTime(500);
    scheduleHideLintProgress(1000);
    vi.advanceTimersByTime(500);

    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(false);

    vi.advanceTimersByTime(500);
    expect(document.querySelector<HTMLElement>("#lint-progress")!.hidden).toBe(true);
    vi.useRealTimers();
  });
});

import { beforeAll, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => false,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: vi.fn(),
}));

import {
  DEFAULT_LINT_CONFIG,
  applyLintConfigToUI,
  applyProjectInfoToUI,
  applyRuleTags,
  applyScriptRootsToUI,
  hideLintProgress,
  lintConfigFromUI,
  openCodeViewer,
  renderMassFixList,
  renderPscResults,
  saveAndCompileCodeViewerEdits,
  saveCodeViewerEdits,
  showLintProgress,
  showResult,
  toggleCodeViewerFullscreen,
  updateLintProgress,
} from "./main";

// main.test.ts exercises the application with the complete index.html-shaped
// fixture. This suite deliberately boots it without that fixture: the same
// situation occurs briefly while the module loads, and can also occur in a
// browser preview whose markup is incomplete. Public UI helpers promise to
// tolerate unavailable optional elements rather than throwing.
beforeAll(() => {
  document.body.replaceChildren();
  document.dispatchEvent(new Event("DOMContentLoaded"));
});

describe("frontend helpers without mounted UI", () => {
  it("reads default lint settings and safely ignores attempts to apply settings", () => {
    expect(lintConfigFromUI()).toEqual(DEFAULT_LINT_CONFIG);
    expect(() => applyLintConfigToUI(DEFAULT_LINT_CONFIG)).not.toThrow();
    expect(() => applyScriptRootsToUI(["/scripts"])).not.toThrow();
    expect(() =>
      applyProjectInfoToUI({ detected_script_roots: ["/project/scripts"], used_configuration_file: null }),
    ).not.toThrow();
    expect(() => applyRuleTags([])).not.toThrow();
  });

  it("makes rendering and progress helpers safe no-ops", () => {
    expect(() => showResult("list.achlist", ["A.psc"], null)).not.toThrow();
    expect(() => renderMassFixList([])).not.toThrow();
    expect(() => renderPscResults([])).not.toThrow();
    expect(() => showLintProgress(2)).not.toThrow();
    expect(() => updateLintProgress(1, 2)).not.toThrow();
    expect(() => hideLintProgress()).not.toThrow();
    expect(() => toggleCodeViewerFullscreen()).not.toThrow();
  });

  it("does not read, save, or compile a file when the code viewer is unavailable", async () => {
    await expect(openCodeViewer("/project/A.psc", [])).resolves.toBeUndefined();
    await expect(saveCodeViewerEdits()).resolves.toBeUndefined();
    await expect(saveAndCompileCodeViewerEdits()).resolves.toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

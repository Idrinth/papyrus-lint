import { beforeAll, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => false,
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: vi.fn(),
}));

import { applyRuleTags, showResult } from "./main";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { applyLintConfigToUI, lintConfigFromUI } from "./config-ui";
import { applyProjectInfoToUI, applyScriptRootsToUI } from "./project-settings";
import { hideLintProgress, showLintActivity, showLintProgress, updateLintProgress } from "./progress";
import { applyAutocompleteSelection, handleAutocompleteKeydown, handleEditorTabKeydown, updateAutocomplete } from "./live-edit-autocomplete";
import { saveAndCompileCodeViewerEdits, saveCodeViewerEdits } from "./live-edit-persist";
import { openCodeViewer, requestCloseCodeViewer, toggleCodeViewerFullscreen } from "./code-viewer-dialog";
import { populateResetPresetSelect, renderPresetManagementTab } from "./presets-management";
import { renderMassFixList } from "./results-list-mass-fix";
import { renderPscResults } from "./results-list-render";
// The per-module UI tests exercise the application with the complete
// index.html-shaped fixture. This suite deliberately boots it without that fixture: the same
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
    expect(() => renderPresetManagementTab([])).not.toThrow();
    expect(() => populateResetPresetSelect([])).not.toThrow();
    expect(() => showLintProgress(2)).not.toThrow();
    expect(() => showLintActivity("Resolving references")).not.toThrow();
    expect(() => updateLintProgress(1, 2)).not.toThrow();
    expect(() => hideLintProgress()).not.toThrow();
    expect(() => toggleCodeViewerFullscreen()).not.toThrow();
  });

  it("makes editor helpers safe no-ops before the editor is mounted", async () => {
    await expect(updateAutocomplete()).resolves.toBeUndefined();
    expect(() => applyAutocompleteSelection(0)).not.toThrow();

    const autocompleteEvent = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(() => handleAutocompleteKeydown(autocompleteEvent)).not.toThrow();
    expect(autocompleteEvent.defaultPrevented).toBe(false);

    const tabEvent = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
    expect(() => handleEditorTabKeydown(tabEvent)).not.toThrow();
    expect(tabEvent.defaultPrevented).toBe(false);
    expect(() => requestCloseCodeViewer()).not.toThrow();
  });

  it("does not read, save, or compile a file when the code viewer is unavailable", async () => {
    await expect(openCodeViewer("/project/A.psc", [])).resolves.toBeUndefined();
    await expect(saveCodeViewerEdits()).resolves.toBeUndefined();
    await expect(saveAndCompileCodeViewerEdits()).resolves.toBeUndefined();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

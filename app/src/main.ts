import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type RuleTagsInfo, loadAppVersion, loadRuleTags } from "./backend";
import { bindPresets, refreshPresetManagementTab } from "./presets";
import { bindCodeViewer, openCodeViewer } from "./code-viewer";
import { bindLiveEdit } from "./live-edit";
import { bindResultsList, populateRuleFilterGroups, renderPscResults } from "./results-list";
import { currentPscOutcomes, handleDroppedPaths, lintResultsStale, relintCurrentFiles } from "./drop";
import { isPscPath, relativePath } from "./path";
import { bindLintProgress } from "./progress";
import { bindConfigSettings } from "./config";
import { bindProjectSettings } from "./project";
import { bindTheme } from "./theme";

let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;

export const TAB_IDS = ["import", "settings", "presets", "files", "lint", "contact"] as const;
type TabId = (typeof TAB_IDS)[number];

// Shows `tab`'s panel and hides the others, updating the tab buttons'
// aria-selected/active state to match.
export function switchTab(tab: TabId) {
  for (const id of TAB_IDS) {
    const button = document.querySelector<HTMLButtonElement>(`#tab-${id}`);
    const panel = document.querySelector<HTMLElement>(`#panel-${id}`);
    const active = id === tab;
    button?.setAttribute("aria-selected", String(active));
    button?.classList.toggle("tabs__tab--active", active);
    if (panel) {
      panel.hidden = !active;
    }
  }
}

// One configuration preset's identity/description — a built-in one, or a
// user preset found under a presets directory next to the executable — as
// returned by the backend's list_config_presets command
// (papyrus_lint_core::presets::PresetInfo, made JSON-friendly). Offered
// inline in the config-picker dialog (see promptForConfigSelection) for a
// project directory that has no papyrus-lint.yaml/.yml of its own yet.
export interface ConfigPreset {
  id: string;
  label: string;
  description: string;
}

// What promptForConfigSelection resolved to (see useProjectDir): stick with
// whatever useProjectDir's own auto-detection would already do ("detected" —
// the project's existing papyrus-lint.yaml/.yml, or the engine's silent
// defaults if it has none), point at a specific configuration file instead
// ("path"), or seed a fresh one from a preset ("preset", handled the same
// way applyConfigPreset already is elsewhere).
export type ConfigSelectionResult =
  | { kind: "detected" }
  | { kind: "path"; path: string }
  | { kind: "preset"; preset: string };

// Every built-in lint rule's tag metadata, keyed by rule id, fetched once
// from the backend (see loadRuleTags) and used both to render each
// finding's tag badges and to drive the tag filters below.
export let ruleTagsByRule: Map<string, RuleTagsInfo> = new Map();

// Diagnostic messages are prefixed with `[level] `; every built-in lint
// tags one, so a message with no recognized prefix never actually occurs in
// practice, but severityOf still classifies it as "error" (matching
// Diagnostic::level()'s own fallback in papyrus-lints/src/lib.rs) rather
// than misclassifying it as something less visible.
export type Severity = "error" | "warning" | "info";
export const SEVERITIES: Severity[] = ["error", "warning", "info"];

export function levelOf(message: string): "error" | "warning" | "info" | null {
  const match = /^\[(error|warning|info)\]/.exec(message);
  return match ? (match[1] as "error" | "warning" | "info") : null;
}

export function escapeAttr(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

export function severityOf(message: string): Severity {
  return levelOf(message) ?? "error";
}

export function showError(message: string) {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = message;
  }
  resultEl?.setAttribute("hidden", "");
  switchTab("import");
}

export function clearError() {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = "";
  }
}

// `base` is the project root (see projectDirForAchlist/projectDirForPscPath),
// used to shorten each entry to a path relative to it so long absolute
// paths stay readable; entries outside `base` (or when it isn't known)
// fall back to their absolute path, per relativePath().
export function showResult(path: string, entries: string[], base: string | null) {
  if (!resultEl || !resultTitleEl || !resultListEl) {
    return;
  }

  resultTitleEl.textContent = `Loaded ${path}`;
  resultListEl.replaceChildren(
    ...entries.map((entry) => {
      const item = document.createElement("li");

      const label = document.createElement("span");
      label.textContent = relativePath(entry, base);
      item.append(label);

      if (isPscPath(entry)) {
        const viewButton = document.createElement("button");
        viewButton.type = "button";
        viewButton.textContent = "View";
        viewButton.classList.add("achlist-result__view-button");
        viewButton.addEventListener("click", () => {
          const outcome = currentPscOutcomes.find((candidate) => candidate.path === entry);
          void openCodeViewer(entry, outcome?.findings ?? []);
        });
        item.append(viewButton);
      }

      return item;
    }),
  );
  resultEl.removeAttribute("hidden");
  switchTab("files");
}

// Indexes `tags` by rule id (for tagsForFinding/matchesTagFilters in
// results-filter.ts), rebuilds each tag kind's "Filter by rule" multiselect
// from the same list, and re-renders the current lint results, so any
// already-listed findings pick up their tag badges/filtering once the
// lookup resolves.
export function applyRuleTags(tags: RuleTagsInfo[]) {
  ruleTagsByRule = new Map(tags.map((info) => [info.rule, info]));
  populateRuleFilterGroups(tags);
  renderPscResults(currentPscOutcomes);
}

window.addEventListener("DOMContentLoaded", () => {
  appVersionEl = document.querySelector("#app-version");
  dropZoneEl = document.querySelector("#drop-zone");
  dropZoneErrorEl = document.querySelector("#drop-zone-error");
  resultEl = document.querySelector("#achlist-result");
  resultTitleEl = document.querySelector("#achlist-result-title");
  resultListEl = document.querySelector("#achlist-result-list");

  bindPresets();
  bindCodeViewer();
  bindLiveEdit();
  bindResultsList();
  bindProjectSettings();
  bindConfigSettings();
  bindLintProgress();
  bindTheme();

  for (const id of TAB_IDS) {
    const button = document.querySelector<HTMLButtonElement>(`#tab-${id}`);
    if (id === "lint") {
      // A settings change since the results currently shown were linted
      // (lintResultsStale) means they no longer reflect the active
      // settings; re-lint the same files instead of just showing the tab.
      button?.addEventListener("click", () => {
        if (lintResultsStale && currentPscOutcomes.length > 0) {
          void relintCurrentFiles();
        } else {
          switchTab("lint");
        }
      });
    } else {
      button?.addEventListener("click", () => switchTab(id));
    }
  }
  switchTab("import");

  // Only reach for the Tauri bridge when actually running inside the
  // desktop app's webview: opened as a plain page (e.g. a browser preview,
  // or the CI Lighthouse check against the built frontend), none of these
  // calls have a backend to talk to and would otherwise throw/log errors.
  if (isTauri()) {
    // The window starts hidden (tauri.conf.json's "visible": false) so the
    // OS never paints its default white background before this styled UI
    // is ready; show it now that theme/tabs are applied, instead of
    // leaving a blank/white window up while Tauri boots on slow machines.
    void getCurrentWindow().show();

    void loadAppVersion().then((version) => {
      if (appVersionEl && version) {
        appVersionEl.textContent = `v${version}`;
      }
    });

    void loadRuleTags().then(applyRuleTags);
    void refreshPresetManagementTab();

    getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        dropZoneEl?.classList.add("drop-zone--active");
      } else if (event.payload.type === "drop") {
        dropZoneEl?.classList.remove("drop-zone--active");
        void handleDroppedPaths(event.payload.paths);
      } else {
        dropZoneEl?.classList.remove("drop-zone--active");
      }
    });
  }
});

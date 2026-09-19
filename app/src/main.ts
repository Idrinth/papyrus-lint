import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type RuleTagsInfo, loadAppVersion, loadRuleTags } from "./backend";
import { bindPresets } from "./presets";
import { refreshPresetManagementTab } from "./presets-management";
import { bindCodeViewer } from "./code-viewer";
import { openCodeViewer } from "./code-viewer-dialog";
import { bindLiveEdit } from "./live-edit";
import { bindResultsList } from "./results-list";
import { renderPscResults } from "./results-list-render";
import { populateRuleFilterGroups, ruleTagsByRule } from "./results-filter";
import { currentPscOutcomes, handleDroppedPaths, lintResultsStale, relintCurrentFiles } from "./drop";
import { isPscPath, relativePath } from "./path";
import { bindLintProgress } from "./progress";
import { bindConfigSettings } from "./config-ui";
import { bindProjectSettings } from "./project-settings";
import { bindTheme } from "./theme";
import { bindWatchMode } from "./watch";
import { TAB_IDS, switchTab } from "./main-tabs";
let appVersionEl: HTMLElement | null;
let dropZoneEl: HTMLElement | null;
let dropZoneLoadingEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;

export { TAB_IDS, switchTab } from "./main-tabs";

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

// Gives immediate feedback while the backend recursively enumerates a dropped
// directory (or reads an achlist), before the Files tab can be populated.
export function setDropZoneLoading(loading: boolean) {
  dropZoneLoadingEl?.toggleAttribute("hidden", !loading);
  dropZoneEl?.setAttribute("aria-busy", String(loading));
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
  ruleTagsByRule.clear();
  for (const info of tags) {
    ruleTagsByRule.set(info.rule, info);
  }
  populateRuleFilterGroups(tags);
  renderPscResults(currentPscOutcomes);
}

window.addEventListener("DOMContentLoaded", () => {
  appVersionEl = document.querySelector("#app-version");
  dropZoneEl = document.querySelector("#drop-zone");
  dropZoneLoadingEl = document.querySelector("#drop-zone-loading");
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
  bindWatchMode();

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

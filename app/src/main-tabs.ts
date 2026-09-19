// Tab identity and switching, extracted from main.ts so presets (and tests)
// can switch tabs without importing the whole UI façade.

export const TAB_IDS = ["import", "settings", "presets", "files", "lint", "contact"] as const;
export type TabId = (typeof TAB_IDS)[number];

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

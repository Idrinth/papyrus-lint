// Tab identity and switching, extracted from main.ts so presets (and tests)
// can switch tabs without importing the whole UI façade.

export const TAB_IDS = ["import", "settings", "presets", "files", "lint", "contact"] as const;
export type TabId = (typeof TAB_IDS)[number];

// Tabs that are always a destination on their own. The others stay hidden
// until something has actually made them usable: a picked project for
// Settings, a user preset for Presets, a successful drop for Files/Lint.
const ALWAYS_AVAILABLE_TABS: ReadonlySet<TabId> = new Set(["import", "contact"]);

function tabButton(id: TabId): HTMLButtonElement | null {
  return document.querySelector<HTMLButtonElement>(`#tab-${id}`);
}

function tabPanel(id: TabId): HTMLElement | null {
  return document.querySelector<HTMLElement>(`#panel-${id}`);
}

function isTabActive(id: TabId): boolean {
  return tabButton(id)?.classList.contains("tabs__tab--active") ?? false;
}

// Shows or hides `tab`'s button. If the tab being hidden is the one
// currently selected, falls back to Import so the user isn't left looking
// at a panel whose button just disappeared. Import and Contact are always
// available; calling this for them is a no-op.
export function setTabAvailable(tab: TabId, available: boolean) {
  if (ALWAYS_AVAILABLE_TABS.has(tab)) {
    return;
  }
  const button = tabButton(tab);
  if (!button) {
    return;
  }
  const wasActive = isTabActive(tab);
  button.hidden = !available;
  if (!available && wasActive) {
    switchTab("import");
  }
}

// Shows `tab`'s panel and hides the others, updating the tab buttons'
// aria-selected/active state to match. Switching to a tab that was still
// hidden also reveals it, so a programmatic switch after a drop (Files,
// Lint) does not land on a destination the user cannot click back to.
export function switchTab(tab: TabId) {
  if (!ALWAYS_AVAILABLE_TABS.has(tab)) {
    const button = tabButton(tab);
    if (button) {
      button.hidden = false;
    }
  }
  for (const id of TAB_IDS) {
    const button = tabButton(id);
    const panel = tabPanel(id);
    const active = id === tab;
    button?.setAttribute("aria-selected", String(active));
    button?.classList.toggle("tabs__tab--active", active);
    if (panel) {
      panel.hidden = !active;
    }
  }
}

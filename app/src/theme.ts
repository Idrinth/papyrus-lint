// Theme persistence: the light/dark/system choice, storing it in
// localStorage, and applying it to the document. bindTheme() wires up the
// #theme-select element itself, called once from main.ts's DOMContentLoaded
// handler.
const THEME_KEY = "papyrus-lint:theme";

export type Theme = "system" | "light" | "dark";
const THEMES: Theme[] = ["system", "light", "dark"];

// Applies `theme` to the document: "system" removes any override, leaving
// the prefers-color-scheme media query in styles.css in control; "light"
// and "dark" set a data-theme attribute that overrides it.
export function applyTheme(theme: Theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

export function storeTheme(theme: Theme) {
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch (error) {
    console.error(error);
  }
}

// Reads the persisted theme choice, defaulting to "system" (also used when
// storage is unavailable or holds something unrecognized).
export function loadStoredTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    return THEMES.includes(stored as Theme) ? (stored as Theme) : "system";
  } catch (error) {
    console.error(error);
    return "system";
  }
}

// Initializes the #theme-select element from the persisted theme, applies
// it, and wires up storing/applying a new one whenever it changes.
export function bindTheme() {
  const themeSelectEl = document.querySelector<HTMLSelectElement>("#theme-select");

  const initialTheme = loadStoredTheme();
  if (themeSelectEl) {
    themeSelectEl.value = initialTheme;
  }
  applyTheme(initialTheme);
  themeSelectEl?.addEventListener("change", () => {
    const theme = (themeSelectEl?.value ?? "system") as Theme;
    storeTheme(theme);
    applyTheme(theme);
  });
}

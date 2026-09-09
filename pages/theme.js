// Wires up the site header's theme <select> to the color theme applied by
// the blocking inline script in each page's <head> (which only handles the
// pre-paint "light"/"dark" override, to avoid a flash of the wrong theme;
// see the inline script for details). This deferred script runs after
// parsing, so it just has to reflect the stored choice into the control and
// react to further changes - the same "system"/"light"/"dark" scheme, and
// the same localStorage key, as the desktop app's own theme switch.
(function () {
  var THEME_KEY = "papyrus-lint:theme";
  var THEMES = ["system", "light", "dark"];

  function loadStoredTheme() {
    try {
      var stored = localStorage.getItem(THEME_KEY);
      return THEMES.indexOf(stored) !== -1 ? stored : "system";
    } catch (error) {
      return "system";
    }
  }

  function applyTheme(theme) {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
  }

  function storeTheme(theme) {
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch (error) {
      // localStorage unavailable (e.g. disabled); the choice just won't
      // persist across page loads.
    }
  }

  function init() {
    var select = document.getElementById("theme-select");
    if (!select) {
      return;
    }
    select.value = loadStoredTheme();
    select.addEventListener("change", function () {
      var theme = THEMES.indexOf(select.value) !== -1 ? select.value : "system";
      storeTheme(theme);
      applyTheme(theme);
    });
  }

  // Keeps --site-header-height in sync with the sticky header's actual
  // rendered height (see styles.css's html { scroll-padding-top }), so an
  // in-page anchor jump (nav links, a direct #fragment URL) lands with the
  // target visible below the header instead of hidden underneath it. A
  // ResizeObserver, not just a window resize listener, is needed because
  // the header's own height can change independent of the viewport (its
  // nav wraps onto more lines at narrower widths, or web fonts swap in).
  function initHeaderHeightTracking() {
    var header = document.querySelector(".site-header");
    if (!header) {
      return;
    }
    function updateHeaderHeight() {
      document.documentElement.style.setProperty("--site-header-height", header.offsetHeight + "px");
    }
    updateHeaderHeight();
    if (typeof ResizeObserver !== "undefined") {
      new ResizeObserver(updateHeaderHeight).observe(header);
    } else {
      window.addEventListener("resize", updateHeaderHeight);
    }
  }

  function initAll() {
    init();
    initHeaderHeightTracking();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initAll);
  } else {
    initAll();
  }
})();

// Progressively enhances the hero's "Download GUI"/"Download CLI" buttons
// (see index.template.html): without JavaScript each is a plain link to the
// latest GitHub release page. With JavaScript, clicking one instead opens a
// small quick-select panel listing that release's actual per-platform
// assets, with the option matching the OS the browser reports pre-selected,
// so most visitors land on the right download without ever seeing the
// release page.
(function () {
  // Every href this script ever sets comes from this literal map, never
  // from the tainted data-options JSON directly: each button's markup only
  // supplies a "file" key naming one of these assets, used purely to look
  // up the matching URL below. A getAttribute()-sourced string reaching a
  // href/src sink is otherwise flagged as a potential DOM XSS by static
  // analysis (CodeQL js/xss-through-dom), regardless of how trustworthy the
  // markup actually is; resolving through this fixed allow-list, rather
  // than validating the untrusted string itself, is what keeps that flow
  // provably safe.
  var DOWNLOAD_URLS = {
    "PapyrusLinter-windows-x64_setup.exe":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-windows-x64_setup.exe",
    "PapyrusLinter-windows-x64.msi":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-windows-x64.msi",
    "PapyrusLinter-darwin-aarch64.dmg":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-darwin-aarch64.dmg",
    "PapyrusLinter-linux-amd64.AppImage":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-linux-amd64.AppImage",
    "PapyrusLinter-linux-amd64.deb":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-linux-amd64.deb",
    "PapyrusLinter-linux-x86_64.rpm":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinter-linux-x86_64.rpm",
    "PapyrusLinterCLI-windows.exe":
      "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinterCLI-windows.exe",
    "PapyrusLinterCLI-macos": "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinterCLI-macos",
    "PapyrusLinterCLI-linux": "https://github.com/idrinth/papyrus-lint/releases/latest/download/PapyrusLinterCLI-linux",
  };

  function detectOS() {
    var ua = (navigator.userAgent || "") + " " + (navigator.platform || "");
    if (/android/i.test(ua)) {
      return null;
    }
    if (/win/i.test(ua)) {
      return "windows";
    }
    if (/mac|iphone|ipad|ipod/i.test(ua)) {
      return "macos";
    }
    if (/linux/i.test(ua)) {
      return "linux";
    }
    return null;
  }

  function closePanel(panel, toggle) {
    panel.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
  }

  function openPanel(panel, toggle) {
    panel.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    var select = panel.querySelector("select");
    if (select) {
      select.focus();
    }
  }

  function enhance(toggle) {
    var raw = toggle.getAttribute("data-options");
    if (!raw) {
      return;
    }

    var options;
    try {
      options = JSON.parse(raw);
    } catch (error) {
      return;
    }
    if (!Array.isArray(options)) {
      return;
    }
    options = options.filter(function (option) {
      return option && Object.prototype.hasOwnProperty.call(DOWNLOAD_URLS, option.file);
    });
    if (options.length === 0) {
      return;
    }

    var os = detectOS();
    var selectedIndex = 0;
    if (os) {
      for (var i = 0; i < options.length; i++) {
        if (options[i].os === os) {
          selectedIndex = i;
          break;
        }
      }
    }

    var group = toggle.closest(".download-group") || toggle.parentNode;
    var id = toggle.getAttribute("data-download-id") || "download";
    var labelText = toggle.getAttribute("data-download-label") || "Choose a download";

    var panel = document.createElement("div");
    panel.className = "download-panel";
    panel.hidden = true;

    var label = document.createElement("label");
    label.className = "visually-hidden";
    label.setAttribute("for", id + "-download-select");
    label.textContent = labelText;

    var select = document.createElement("select");
    select.id = id + "-download-select";
    select.className = "download-panel__select";
    options.forEach(function (option) {
      var el = document.createElement("option");
      el.value = option.file;
      el.textContent = option.label;
      select.appendChild(el);
    });
    select.selectedIndex = selectedIndex;

    var go = document.createElement("a");
    go.className = "button button--primary download-panel__go";
    go.textContent = "Download";
    go.href = DOWNLOAD_URLS[options[selectedIndex].file];

    select.addEventListener("change", function () {
      var url = DOWNLOAD_URLS[select.value];
      if (url) {
        go.href = url;
      }
    });

    panel.appendChild(label);
    panel.appendChild(select);
    panel.appendChild(go);
    group.appendChild(panel);

    toggle.classList.add("button--has-menu");
    toggle.setAttribute("aria-haspopup", "true");
    toggle.setAttribute("aria-expanded", "false");
    toggle.addEventListener("click", function (event) {
      event.preventDefault();
      if (panel.hidden) {
        openPanel(panel, toggle);
      } else {
        closePanel(panel, toggle);
      }
    });

    document.addEventListener("click", function (event) {
      if (!panel.hidden && !group.contains(event.target)) {
        closePanel(panel, toggle);
      }
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && !panel.hidden) {
        closePanel(panel, toggle);
        toggle.focus();
      }
    });
  }

  function init() {
    var toggles = document.querySelectorAll("[data-download-toggle]");
    toggles.forEach(enhance);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();

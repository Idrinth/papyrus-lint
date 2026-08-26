import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;

const ARCHLIST_EXTENSION = ".archlist";

function isArchlistPath(path: string): boolean {
  return path.toLowerCase().endsWith(ARCHLIST_EXTENSION);
}

function showError(message: string) {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = message;
  }
  resultEl?.setAttribute("hidden", "");
}

function clearError() {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = "";
  }
}

function showResult(path: string, entries: string[]) {
  if (!resultEl || !resultTitleEl || !resultListEl) {
    return;
  }

  resultTitleEl.textContent = `Loaded ${path}`;
  resultListEl.replaceChildren(
    ...entries.map((entry) => {
      const item = document.createElement("li");
      item.textContent = entry;
      return item;
    }),
  );
  resultEl.removeAttribute("hidden");
}

async function handleDroppedPaths(paths: string[]) {
  const archlistPath = paths.find(isArchlistPath);

  if (!archlistPath) {
    showError("Please drop a single .archlist file.");
    return;
  }

  try {
    const entries = await invoke<string[]>("parse_archlist_file", {
      path: archlistPath,
    });
    clearError();
    showResult(archlistPath, entries);
  } catch (error) {
    showError("Failed to read that .archlist file. Please try again.");
    console.error(error);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  dropZoneEl = document.querySelector("#drop-zone");
  dropZoneErrorEl = document.querySelector("#drop-zone-error");
  resultEl = document.querySelector("#archlist-result");
  resultTitleEl = document.querySelector("#archlist-result-title");
  resultListEl = document.querySelector("#archlist-result-list");

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
});

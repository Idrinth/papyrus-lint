import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
let pscResultEl: HTMLElement | null;
let pscResultListEl: HTMLElement | null;

const ARCHLIST_EXTENSION = ".archlist";
const PSC_EXTENSION = ".psc";

function isArchlistPath(path: string): boolean {
  return path.toLowerCase().endsWith(ARCHLIST_EXTENSION);
}

function isPscPath(path: string): boolean {
  return path.toLowerCase().endsWith(PSC_EXTENSION);
}

interface PapyrusScript {
  name: string;
}

type LintLevel = "error" | "warning" | "info";

interface LintFinding {
  line: number;
  level: LintLevel;
  script: string;
  function: string;
  message: string;
}

interface PscParseOutcome {
  path: string;
  ok: boolean;
  detail: string;
  findings: LintFinding[];
}

async function lintPscFile(path: string): Promise<LintFinding[]> {
  try {
    return await invoke<LintFinding[]>("lint_psc_file", { path });
  } catch (error) {
    console.error(error);
    return [];
  }
}

async function parsePscFiles(paths: string[]): Promise<PscParseOutcome[]> {
  return Promise.all(
    paths.map(async (path) => {
      try {
        const script = await invoke<PapyrusScript>("parse_psc_file", { path });
        const findings = await lintPscFile(path);
        return { path, ok: true, detail: `parsed as "${script.name}"`, findings };
      } catch (error) {
        return { path, ok: false, detail: String(error), findings: [] };
      }
    }),
  );
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

function showPscResults(outcomes: PscParseOutcome[]) {
  if (!pscResultEl || !pscResultListEl) {
    return;
  }

  if (outcomes.length === 0) {
    pscResultEl.setAttribute("hidden", "");
    return;
  }

  pscResultListEl.replaceChildren(
    ...outcomes.map(({ path, ok, detail, findings }) => {
      const item = document.createElement("li");
      item.textContent = `${path}: ${detail}`;
      item.classList.add(ok ? "psc-result__item--ok" : "psc-result__item--error");

      if (findings.length > 0) {
        const findingsList = document.createElement("ul");
        findingsList.classList.add("psc-result__findings");
        findingsList.replaceChildren(
          ...findings.map((finding) => {
            const findingItem = document.createElement("li");
            findingItem.textContent = `line ${finding.line}: [${finding.level}] ${finding.script}.${finding.function} — ${finding.message}`;
            findingItem.classList.add(`psc-result__finding--${finding.level}`);
            return findingItem;
          }),
        );
        item.append(findingsList);
      }

      return item;
    }),
  );
  pscResultEl.removeAttribute("hidden");
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

    const outcomes = await parsePscFiles(entries.filter(isPscPath));
    showPscResults(outcomes);
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
  pscResultEl = document.querySelector("#psc-result");
  pscResultListEl = document.querySelector("#psc-result-list");

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

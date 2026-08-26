import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

let dropZoneEl: HTMLElement | null;
let dropZoneErrorEl: HTMLElement | null;
let resultEl: HTMLElement | null;
let resultTitleEl: HTMLElement | null;
let resultListEl: HTMLElement | null;
let pscResultEl: HTMLElement | null;
let pscResultListEl: HTMLElement | null;
let indentationStyleEl: HTMLSelectElement | null;
let indentationWidthEl: HTMLInputElement | null;
let currentPscOutcomes: PscParseOutcome[] = [];
let semicolonStyleEl: HTMLSelectElement | null;

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

interface Diagnostic {
  line: number;
  column: number;
  message: string;
}

interface PscParseOutcome {
  path: string;
  ok: boolean;
  detail: string;
  findings: Diagnostic[];
}

type Indentation = "Tabs" | { Spaces: number };
interface LintConfig {
  semicolon: boolean;
  indentation: "tab" | "space";
}

const DEFAULT_LINT_CONFIG: LintConfig = { semicolon: false, indentation: "tab" };

let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;

const TRAILING_WHITESPACE_MESSAGE = "Line contains trailing whitespace";
type SemicolonStyle = "require" | "forbid";

function semicolonStyle(): SemicolonStyle {
  return (semicolonStyleEl?.value as SemicolonStyle | undefined) ?? "forbid";
}

function dirnameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(0, index);
}

// Looks for a papyrus-lint YAML config file next to the dropped .archlist
// file, falling back to the default configuration if none is found.
async function loadLintConfig(archlistPath: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config", { dir: dirnameOf(archlistPath) });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", { path, semicolonStyle: semicolonStyle() });
    return await invoke<Diagnostic[]>("lint_psc_file", { path, config: currentLintConfig });
  } catch (error) {
    console.error(error);
    return [];
  }
}

async function repairPscFile(path: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file", { path, semicolonStyle: semicolonStyle() });
  const indentation: Indentation =
    indentationStyleEl?.value === "spaces"
      ? { Spaces: Math.min(16, Math.max(1, indentationWidthEl?.valueAsNumber || 4)) }
      : "Tabs";
  return invoke<Diagnostic[]>("repair_psc_file", { path, indentation });
  return invoke<Diagnostic[]>("repair_psc_file", { path, config: currentLintConfig });
}

function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) =>
    finding.message === TRAILING_WHITESPACE_MESSAGE || finding.message.includes("end with a semicolon"),
  );
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

// Diagnostic messages are prefixed with `[level] ` by the lints that care
// about severity (e.g. `forbidden_functions`); others have no prefix.
function levelOf(message: string): "error" | "warning" | "info" | null {
  const match = /^\[(error|warning|info)\]/.exec(message);
  return match ? (match[1] as "error" | "warning" | "info") : null;
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

function renderPscResults(outcomes: PscParseOutcome[]) {
  if (!pscResultEl || !pscResultListEl) {
    return;
  }

  if (outcomes.length === 0) {
    pscResultEl.setAttribute("hidden", "");
    return;
  }

  pscResultListEl.replaceChildren(
    ...outcomes.map((outcome) => {
      const { path, ok, detail, findings } = outcome;
      const item = document.createElement("li");
      item.classList.add(ok ? "psc-result__item--ok" : "psc-result__item--error");

      const summary = document.createElement("span");
      summary.textContent = `${path}: ${detail}`;
      item.append(summary);

      if (hasFixableFindings(findings)) {
        const fixButton = document.createElement("button");
        fixButton.type = "button";
        fixButton.textContent = "Apply fixes";
        fixButton.classList.add("psc-result__fix-button");
        fixButton.addEventListener("click", () => void handleFixClick(path, outcome, fixButton));
        item.append(fixButton);
      }
      const fixButton = document.createElement("button");
      fixButton.type = "button";
      fixButton.textContent = "Apply fixes";
      fixButton.classList.add("psc-result__fix-button");
      fixButton.addEventListener("click", () => void handleFixClick(path, outcome, fixButton));
      item.append(fixButton);

      if (findings.length > 0) {
        const findingsList = document.createElement("ul");
        findingsList.classList.add("psc-result__findings");
        findingsList.replaceChildren(
          ...findings.map((finding) => {
            const findingItem = document.createElement("li");
            findingItem.textContent = `line ${finding.line}, col ${finding.column}: ${finding.message}`;
            const level = levelOf(finding.message);
            if (level) {
              findingItem.classList.add(`psc-result__finding--${level}`);
            }
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

async function handleFixClick(path: string, outcome: PscParseOutcome, button: HTMLButtonElement) {
  button.disabled = true;
  try {
    outcome.findings = await repairPscFile(path);
  } catch (error) {
    console.error(error);
  } finally {
    renderPscResults(currentPscOutcomes);
  }
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

    currentLintConfig = await loadLintConfig(archlistPath);
    currentPscOutcomes = await parsePscFiles(entries.filter(isPscPath));
    renderPscResults(currentPscOutcomes);
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
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
  });

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

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

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

function isAchlistPath(path: string): boolean {
  return path.toLowerCase().endsWith(ACHLIST_EXTENSION);
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

interface LintConfig {
  semicolon: boolean;
  indentation: "tab" | "space";
  indentation_width: number;
}

const DEFAULT_LINT_CONFIG: LintConfig = { semicolon: false, indentation: "tab", indentation_width: 4 };
const LAST_PROJECT_DIR_KEY = "papyrus-lint:last-project-dir";

let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;
// The project root (the directory containing the dropped .achlist file),
// also used by the "Argument type check" lint to resolve calls to
// functions declared on other scripts under it.
let currentProjectDir: string | null = null;

const TRAILING_WHITESPACE_MESSAGE = "Line contains trailing whitespace";

function dirnameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(0, index);
}

// Looks for a papyrus-lint YAML config file in `dir`, falling back to the
// default configuration if none is found.
async function loadLintConfig(dir: string): Promise<LintConfig> {
  try {
    return await invoke<LintConfig>("load_lint_config", { dir });
  } catch (error) {
    console.error(error);
    return DEFAULT_LINT_CONFIG;
  }
}

// Persists `config` to `dir`'s papyrus-lint YAML config file so the
// formatting selected in the UI is remembered for next time.
async function saveLintConfig(dir: string, config: LintConfig): Promise<void> {
  try {
    await invoke("save_lint_config", { dir, config });
  } catch (error) {
    console.error(error);
  }
}

// Reflects `config` onto the formatting controls without firing their
// `change` listeners (assigning `.value` does not dispatch `change`).
function applyLintConfigToUI(config: LintConfig) {
  if (semicolonStyleEl) {
    semicolonStyleEl.value = config.semicolon ? "require" : "forbid";
  }
  if (indentationStyleEl) {
    indentationStyleEl.value = config.indentation === "space" ? "spaces" : "tabs";
  }
  if (indentationWidthEl) {
    indentationWidthEl.value = String(config.indentation_width);
    indentationWidthEl.disabled = config.indentation !== "space";
  }
}

// Reads the formatting controls' current values into a LintConfig.
function lintConfigFromUI(): LintConfig {
  const indentation = indentationStyleEl?.value === "spaces" ? "space" : "tab";
  return {
    semicolon: semicolonStyleEl?.value === "require",
    indentation,
    indentation_width: Math.min(16, Math.max(1, indentationWidthEl?.valueAsNumber || 4)),
  };
}

// Called whenever a formatting control changes: updates the in-memory
// config and, if a project directory is known, persists it to disk.
function handleLintConfigChanged() {
  currentLintConfig = lintConfigFromUI();
  if (currentProjectDir) {
    void saveLintConfig(currentProjectDir, currentLintConfig);
  }
}

async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      root: currentProjectDir ?? "",
      config: currentLintConfig,
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

async function repairPscFile(path: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file", {
    path,
    root: currentProjectDir ?? "",
    config: currentLintConfig,
  });
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

// Remembers `dir` as the last project opened, so its config file can be
// read again the next time the app starts.
function rememberProjectDir(dir: string) {
  try {
    localStorage.setItem(LAST_PROJECT_DIR_KEY, dir);
  } catch (error) {
    console.error(error);
  }
}

function lastProjectDir(): string | null {
  try {
    return localStorage.getItem(LAST_PROJECT_DIR_KEY);
  } catch (error) {
    console.error(error);
    return null;
  }
}

async function useProjectDir(dir: string) {
  currentProjectDir = dir;
  currentLintConfig = await loadLintConfig(dir);
  applyLintConfigToUI(currentLintConfig);
  rememberProjectDir(dir);
}

async function handleDroppedPaths(paths: string[]) {
  const achlistPath = paths.find(isAchlistPath);

  if (!achlistPath) {
    showError("Please drop a single .achlist file.");
    return;
  }

  try {
    const entries = await invoke<string[]>("parse_achlist_file", {
      path: achlistPath,
    });
    clearError();
    showResult(achlistPath, entries);

    await useProjectDir(dirnameOf(achlistPath));
    currentPscOutcomes = await parsePscFiles(entries.filter(isPscPath));
    renderPscResults(currentPscOutcomes);
  } catch (error) {
    showError("Failed to read that .achlist file. Please try again.");
    console.error(error);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  dropZoneEl = document.querySelector("#drop-zone");
  dropZoneErrorEl = document.querySelector("#drop-zone-error");
  resultEl = document.querySelector("#achlist-result");
  resultTitleEl = document.querySelector("#achlist-result-title");
  resultListEl = document.querySelector("#achlist-result-list");
  pscResultEl = document.querySelector("#psc-result");
  pscResultListEl = document.querySelector("#psc-result-list");
  semicolonStyleEl = document.querySelector("#semicolon-style");
  indentationStyleEl = document.querySelector("#indentation-style");
  indentationWidthEl = document.querySelector("#indentation-width");

  semicolonStyleEl?.addEventListener("change", handleLintConfigChanged);
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
    handleLintConfigChanged();
  });
  indentationWidthEl?.addEventListener("change", handleLintConfigChanged);

  const lastDir = lastProjectDir();
  if (lastDir) {
    void useProjectDir(lastDir);
  }

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

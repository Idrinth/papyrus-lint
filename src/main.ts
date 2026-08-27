import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { highlightPapyrusLines } from "./highlight";

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
let codeViewerEl: HTMLDialogElement | null;
let codeViewerTitleEl: HTMLElement | null;
let codeViewerBodyEl: HTMLElement | null;
let codeViewerCloseEl: HTMLButtonElement | null;

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

const TAB_IDS = ["import", "settings", "files", "lint"] as const;
type TabId = (typeof TAB_IDS)[number];

// Shows `tab`'s panel and hides the others, updating the tab buttons'
// aria-selected/active state to match.
function switchTab(tab: TabId) {
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

function escapeAttr(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

// Reads and syntax-highlights `path`'s source, then opens the code viewer
// dialog with `findings` marked on their lines. If `focusLine` is given,
// scrolls that line into view and briefly flashes it, so a click on a
// specific finding jumps straight to it.
async function openCodeViewer(path: string, findings: Diagnostic[], focusLine?: number) {
  if (!codeViewerEl || !codeViewerTitleEl || !codeViewerBodyEl) {
    return;
  }

  codeViewerTitleEl.textContent = path;
  codeViewerBodyEl.textContent = "Loading…";
  codeViewerEl.showModal();

  let source: string;
  try {
    source = await invoke<string>("read_psc_file", { path });
  } catch (error) {
    codeViewerBodyEl.textContent = `Failed to read file: ${String(error)}`;
    return;
  }

  const findingsByLine = new Map<number, Diagnostic[]>();
  for (const finding of findings) {
    const forLine = findingsByLine.get(finding.line) ?? [];
    forLine.push(finding);
    findingsByLine.set(finding.line, forLine);
  }

  function severityOf(lineFindings: Diagnostic[] | undefined): "error" | "warning" | "info" | "flagged" | null {
    if (!lineFindings || lineFindings.length === 0) {
      return null;
    }
    const levels = lineFindings.map((finding) => levelOf(finding.message));
    if (levels.includes("error")) return "error";
    if (levels.includes("warning")) return "warning";
    if (levels.includes("info")) return "info";
    // Lints like trailing-whitespace don't tag a severity level; still mark
    // their line so the finding is visible in the viewer.
    return "flagged";
  }

  const lines = highlightPapyrusLines(source);
  const rows = lines.map((lineHtml, index) => {
    const lineNumber = index + 1;
    const lineFindings = findingsByLine.get(lineNumber);
    const severity = severityOf(lineFindings);
    const rowClass = severity ? ` class="code-viewer__line--${severity}"` : "";
    const title = lineFindings
      ? ` title="${escapeAttr(lineFindings.map((f) => f.message).join("\n"))}"`
      : "";
    return (
      `<tr id="code-viewer-line-${lineNumber}"${rowClass}${title}>` +
      `<td class="code-viewer__line-number">${lineNumber}</td>` +
      `<td class="code-viewer__line-code">${lineHtml}</td>` +
      `</tr>`
    );
  });

  codeViewerBodyEl.innerHTML = `<table class="code-viewer__table"><tbody>${rows.join("")}</tbody></table>`;

  if (focusLine) {
    const row = codeViewerBodyEl.querySelector<HTMLElement>(`#code-viewer-line-${focusLine}`);
    row?.scrollIntoView({ block: "center" });
    row?.classList.add("code-viewer__line--flash");
  }
}

function showError(message: string) {
  if (dropZoneErrorEl) {
    dropZoneErrorEl.textContent = message;
  }
  resultEl?.setAttribute("hidden", "");
  switchTab("import");
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
  switchTab("files");
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

      const viewButton = document.createElement("button");
      viewButton.type = "button";
      viewButton.textContent = "View code";
      viewButton.classList.add("psc-result__view-button");
      viewButton.addEventListener("click", () => void openCodeViewer(path, outcome.findings));
      item.append(viewButton);

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
            findingItem.classList.add("psc-result__finding");
            const level = levelOf(finding.message);
            if (level) {
              findingItem.classList.add(`psc-result__finding--${level}`);
            }
            findingItem.addEventListener("click", () => void openCodeViewer(path, outcome.findings, finding.line));
            return findingItem;
          }),
        );
        item.append(findingsList);
      }

      return item;
    }),
  );
  pscResultEl.removeAttribute("hidden");
  switchTab("lint");
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
  codeViewerEl = document.querySelector("#code-viewer");
  codeViewerTitleEl = document.querySelector("#code-viewer-title");
  codeViewerBodyEl = document.querySelector("#code-viewer-body");
  codeViewerCloseEl = document.querySelector("#code-viewer-close");

  codeViewerCloseEl?.addEventListener("click", () => codeViewerEl?.close());
  codeViewerEl?.addEventListener("click", (event) => {
    if (event.target === codeViewerEl) {
      codeViewerEl?.close();
    }
  });

  semicolonStyleEl?.addEventListener("change", handleLintConfigChanged);
  indentationStyleEl?.addEventListener("change", () => {
    if (indentationWidthEl) {
      indentationWidthEl.disabled = indentationStyleEl?.value !== "spaces";
    }
    handleLintConfigChanged();
  });
  indentationWidthEl?.addEventListener("change", handleLintConfigChanged);

  for (const id of TAB_IDS) {
    document.querySelector<HTMLButtonElement>(`#tab-${id}`)?.addEventListener("click", () => switchTab(id));
  }
  switchTab("import");

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

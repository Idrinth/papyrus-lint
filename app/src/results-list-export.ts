import { type PscParseOutcome, loadAppVersion } from "./backend";
import { type LintConfig, currentLintConfig } from "./config-types";
import { currentPscOutcomes } from "./drop";
import { downloadTextFile } from "./download-text-file";
import { formatIssuesForAi as formatIssuesForAiDocument, readIssueFileSources } from "./results-export-ai";
import { formatIssuesAsJson } from "./results-export-json";
import { formatIssuesAsText } from "./results-export-text";
import { type AiSource, type FilteredIssuesFile } from "./results-export-types";
import { activeFiltersForExport, collectFilteredIssues } from "./results-filter";
import { exportAiButtonEl, exportAiHashSourceEl, exportFormatEl, exportIssuesButtonEl } from "./results-list-state";
export async function formatIssuesForAi(
  files: FilteredIssuesFile[],
  version: string,
  sources: Map<string, AiSource> = new Map(),
  configuration: LintConfig = currentLintConfig,
): Promise<string> {
  return formatIssuesForAiDocument(files, version, sources, configuration, activeFiltersForExport());
}

// Enables the "Export issues"/"Export for AI" buttons only while there's at
// least one currently filtered finding to export.
export function updateExportIssuesButtonState(outcomes: PscParseOutcome[]) {
  const disabled = collectFilteredIssues(outcomes).length === 0;
  if (exportIssuesButtonEl) {
    exportIssuesButtonEl.disabled = disabled;
  }
  if (exportAiButtonEl) {
    exportAiButtonEl.disabled = disabled;
  }
}

// Downloads the currently filtered lint findings (see collectFilteredIssues)
// as a single text or JSON file, per the "Export format" selector.
export async function handleExportIssuesClick(): Promise<void> {
  const files = collectFilteredIssues(currentPscOutcomes);
  if (files.length === 0) {
    return;
  }
  if (exportFormatEl?.value === "json") {
    downloadTextFile("papyrus-lint-issues.json", await formatIssuesAsJson(files), "application/json");
  } else {
    downloadTextFile("papyrus-lint-issues.txt", await formatIssuesAsText(files), "text/plain");
  }
}

// Downloads the currently filtered lint findings as a single "Export for
// AI" JSON document (see formatIssuesForAi), each file's current source
// attached (see readIssueFileSources) - as an md5 hash instead of its full
// content when the "Redact source" checkbox is checked - independent of the
// "Export format" selector above since this format is always JSON.
export async function handleExportAiClick(): Promise<void> {
  const files = collectFilteredIssues(currentPscOutcomes);
  if (files.length === 0) {
    return;
  }
  const hashSource = exportAiHashSourceEl?.checked ?? false;
  const [version, sources] = await Promise.all([
    loadAppVersion(),
    readIssueFileSources(files, currentPscOutcomes, hashSource),
  ]);
  downloadTextFile(
    "papyrus-lint-ai-export.json",
    await formatIssuesForAi(files, version, sources),
    "application/json",
  );
}

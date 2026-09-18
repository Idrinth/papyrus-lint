import { invoke } from "@tauri-apps/api/core";
import { type PscParseOutcome, FIXABLE_RULE_IDS, hasNoAutomaticFix, previewRepairPscLine } from "./backend";
import { type LintConfig } from "./config";
import { currentProjectDir } from "./project";
import { relativePath } from "./path";
import { type ActiveFilters, type AiSource, type FilteredIssuesFile, toIssuesFileInput } from "./results-export-types";
import { sortedByPosition } from "./results-export-json";

// The AI export's own `configuration` shape (see formatIssuesForAi below):
// the same resolved LintConfig a lint run used, except its `rules` object
// (58 individual enable flags, each with its own description in the
// schema) is replaced with a compact, alphabetically sorted
// `enabled_rules` list of just the hyphenated ids that are currently on -
// no information is lost, since a rule absent from the list is simply
// disabled, but every export no longer repeats a large, mostly-constant
// block of booleans. Mirrors papyrus_lint_output::ai_configuration, used by
// the format_issues_for_ai_base Tauri command below for every field except
// `filters`, which is GUI-only and layered on afterward here.
export function aiConfiguration(config: LintConfig): Record<string, unknown> {
  const { rules, ...rest } = config;
  const enabledRules = Object.entries(rules)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name.replace(/_/g, "-"))
    .sort((a, b) => a.localeCompare(b));
  return { ...rest, enabled_rules: enabledRules };
}

// Reads each of `files`' current on-disk source (or, with `hashSource`, just
// its md5 digest - see AiSource above) via the read_psc_file/hash_psc_file_md5
// commands the code viewer and this redaction option use respectively, keyed
// by each file's display path (see FilteredIssuesFile.path) for
// formatIssuesForAi below to attach alongside that file's findings - so an AI
// reasoning about the report can see the actual surrounding code a
// diagnostic refers to (or, redacted, at least confirm which version of a
// file it's looking at) without opening the project itself. `outcomes`
// supplies the absolute path either command needs, matched back to `files`'
// relative display path via the same relativePath()/currentProjectDir
// pairing collectFilteredIssues used to produce it in the first place. A
// read/hash failure (e.g. the file was moved or deleted since linting)
// records an `error` entry instead of failing the whole export, since the
// rest of the report stays useful without it.
export async function readIssueFileSources(
  files: FilteredIssuesFile[],
  outcomes: PscParseOutcome[],
  hashSource: boolean,
): Promise<Map<string, AiSource>> {
  const absolutePathsByDisplayPath = new Map<string, string>();
  for (const outcome of outcomes) {
    absolutePathsByDisplayPath.set(relativePath(outcome.path, currentProjectDir), outcome.path);
  }
  const sources = new Map<string, AiSource>();
  await Promise.all(
    files.map(async (file) => {
      const absolutePath = absolutePathsByDisplayPath.get(file.path) ?? file.path;
      try {
        if (hashSource) {
          const hash = await invoke<string>("hash_psc_file_md5", { path: absolutePath });
          sources.set(file.path, { type: "hash", algorithm: "md5", hash });
        } else {
          const content = await invoke<string>("read_psc_file", { path: absolutePath });
          sources.set(file.path, { type: "content", content });
        }
      } catch (error) {
        sources.set(file.path, { type: "error", message: String(error) });
      }
    }),
  );
  return sources;
}

// The rule id papyrus_lints::Diagnostics never define themselves:
// app/src-tauri/src/compile_diagnostics.rs's own `RULE` constant, attached
// to a diagnostic parsed out of PapyrusCompiler.exe's own error output
// (see lint_with_compile_check in app/src-tauri/src/lint.rs) rather than
// raised by one of Papyrus Lint's own lint rules. formatIssuesForAi below
// uses it to flag such a diagnostic as external in the AI export, since an
// assistant reading the export otherwise has no way to tell a
// compiler-reported syntax error apart from an ordinary lint finding.
// Kept in sync by hand with that Rust constant, the same convention
// FIXABLE_RULE_IDS follows for papyrus_lints::FIXABLE_RULE_IDS.
const COMPILER_ERROR_RULE = "compiler-error";

// One diagnostic entry as returned by the format_issues_for_ai_base Tauri
// command's own findings.files[].diagnostics - the subset of fields this
// module needs to decorate each one with a repair preview / external flag
// below, ignoring the rest (line, column, doc_url, ...) it just passes
// through untouched.
interface AiBaseDiagnostic {
  rule: string;
  level: string;
  [key: string]: unknown;
}

interface AiBaseFile {
  path: string;
  diagnostics: AiBaseDiagnostic[];
  [key: string]: unknown;
}

interface AiBaseReport {
  findings: { files: AiBaseFile[]; [key: string]: unknown };
  [key: string]: unknown;
}

// Renders `files` as a single JSON document meant to be handed to an AI
// assistant alongside a question about the results. The shared header,
// resolved configuration, per-file/per-report diagnostic counts, and
// triggered-rule details come from the format_issues_for_ai_base Tauri
// command (app/src-tauri/src/export.rs), built on the same
// papyrus-lint-output crate the CLI's own `--format ai` uses; this function
// then layers on what only the GUI has: the currently active result
// filters (`filters`, see ActiveFilters), an `external: true`/`source:
// "compiler"` pair on every diagnostic raised by PapyrusCompiler.exe itself
// rather than one of Papyrus Lint's own rules (see COMPILER_ERROR_RULE
// above), and a `repair` field on every diagnostic from an auto-fixable
// rule Papyrus Lint could compute a fix preview for (see
// previewRepairPscLine; omitted when the rule doesn't actually change that
// line, e.g. type-casing's "no automatic fix" case, or its fix would shift
// the file's line count elsewhere). `version` is the running app's version
// (see loadAppVersion), or "" if that lookup failed.
export async function formatIssuesForAi(
  files: FilteredIssuesFile[],
  version: string,
  sources: Map<string, AiSource> = new Map(),
  configuration: LintConfig,
  filters: ActiveFilters,
): Promise<string> {
  const sortedFiles = sortedByPosition(files);

  const base = JSON.parse(
    await invoke<string>("format_issues_for_ai_base", {
      files: toIssuesFileInput(sortedFiles).map((file, fileIndex) => ({
        ...file,
        source: sources.get(sortedFiles[fileIndex].path) ?? null,
      })),
      configuration: configuration,
      version: version || "unknown",
    }),
  ) as AiBaseReport;

  base.filters = filters;

  await Promise.all(
    base.findings.files.map(async (fileReport, fileIndex) => {
      const sortedFindings = sortedFiles[fileIndex].findings;
      await Promise.all(
        fileReport.diagnostics.map(async (diagnostic, diagnosticIndex) => {
          const finding = sortedFindings[diagnosticIndex];
          if (diagnostic.rule === COMPILER_ERROR_RULE) {
            diagnostic.external = true;
            diagnostic.source = "compiler";
          }
          if (!finding.rule || !FIXABLE_RULE_IDS.has(finding.rule) || hasNoAutomaticFix(finding)) {
            return;
          }
          const repair = await previewRepairPscLine(fileReport.path, finding.rule, finding.line);
          if (repair !== null) {
            diagnostic.repair = repair;
          }
        }),
      );
    }),
  );

  return JSON.stringify(base, null, 2);
}

import { invoke } from "@tauri-apps/api/core";
import {
  type PscParseOutcome,
  type RuleTagsInfo,
  type Severity,
  FIXABLE_RULE_IDS,
  hasNoAutomaticFix,
  previewRepairPscLine,
  ruleTagsByRule,
} from "./main";
import { type LintConfig } from "./config";
import { currentProjectDir } from "./project";
import { relativePath } from "./path";
import { type ActiveFilters, type AiSource, type FilteredIssuesFile } from "./results-export-types";
import { buildIssuesReport, sortedByPosition } from "./results-export-json";

// The AI export's own `configuration` shape (see formatIssuesForAi below):
// the same resolved LintConfig a lint run used, except its `rules` object
// (58 individual enable flags, each with its own description in the
// schema) is replaced with a compact, alphabetically sorted
// `enabled_rules` list of just the hyphenated ids that are currently on -
// no information is lost, since a rule absent from the list is simply
// disabled, but every export no longer repeats a large, mostly-constant
// block of booleans. Mirrors papyrus_lints::Rules::enabled_ids and
// papyrus-lint-cli's own ai_configuration in app/crates/papyrus-lint-cli/src/lib.rs.
export function aiConfiguration(config: LintConfig): Record<string, unknown> {
  const { rules, ...rest } = config;
  const enabledRules = Object.entries(rules)
    .filter(([, enabled]) => enabled)
    .map(([name]) => name.replace(/_/g, "-"))
    .sort((a, b) => a.localeCompare(b));
  return { ...rest, enabled_rules: enabledRules };
}

// The desktop app's own homepage, where an AI reading an "Export for AI"
// document (see formatIssuesForAi) can look up rule/configuration
// documentation beyond what rule_details itself carries.
const WEBSITE_URL = "https://papyrus-lint.idrinth.de";
const AI_EXPORT_SCHEMA_URL =
  "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json";
const TOOL_NAME = "Papyrus Lint";
// The Papyrus dialect/engine version these findings were produced for, so
// an AI reading the export doesn't have to guess whether a suggestion (e.g.
// referencing a native type only added in a later game/edition) actually
// applies. Papyrus Lint has no per-project game/edition setting of its own
// (see rules/native-types.yaml's shared Skyrim/Fallout 4 fallback), so this
// is the fixed target its native rule data is written against.
const TARGET_GAME = "Skyrim SE/AE";

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

// Tallies `diagnostics` by rule id, alphabetically sorted, for the
// per-file and report-wide `rule_counts` fields in formatIssuesForAi's
// output.
function ruleCounts(diagnostics: { rule: string }[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const diagnostic of diagnostics) {
    counts[diagnostic.rule] = (counts[diagnostic.rule] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

// Tallies `diagnostics` by severity level, for the per-file and
// report-wide `severity_counts` fields in formatIssuesForAi's output.
function severityCounts(diagnostics: { level: Severity }[]): { errors: number; warnings: number; info: number } {
  return {
    errors: diagnostics.filter((diagnostic) => diagnostic.level === "error").length,
    warnings: diagnostics.filter((diagnostic) => diagnostic.level === "warning").length,
    info: diagnostics.filter((diagnostic) => diagnostic.level === "info").length,
  };
}

// Renders `files` as a single JSON document meant to be handed to an AI
// assistant alongside a question about the results: a header identifying
// the tool/version/website/target game and generation time (so the AI knows what produced
// these findings and where to look up anything not covered below), the
// findings themselves (see buildIssuesReport) with each file's `source`
// field set to whatever `sources` has for it - one of the four explicit
// AiSource shapes (see readIssueFileSources), or `null` when `sources` has
// no entry for that file at all - an `external: true`/`source: "compiler"`
// pair added to every diagnostic raised by PapyrusCompiler.exe itself
// rather than one of Papyrus Lint's own rules (see COMPILER_ERROR_RULE
// above), so the assistant can tell a compiler-reported error apart from
// an ordinary lint finding - a `repair` field added to every
// diagnostic from an auto-fixable rule Papyrus Lint could compute a fix
// preview for (see previewRepairPscLine; omitted when the rule doesn't
// actually change that line, e.g. type-casing's "no automatic fix" case, or
// its fix would shift the file's line count elsewhere), and the full tag
// metadata (kind(s), importance, and the rule's detailed
// description copied from its README.md row; see papyrus_lints::tags) for
// every rule id that actually appears among `files`' findings - giving the
// AI enough context about each triggered rule, in the same detail the
// README gives a human reader, the actual code each diagnostic refers to,
// and what its fix would look like, to answer follow-up questions precisely
// without needing the project's own files or documentation on hand.
// `version` is the running app's version (see loadAppVersion), or "" if
// that lookup failed.
export async function formatIssuesForAi(
  files: FilteredIssuesFile[],
  version: string,
  sources: Map<string, AiSource>,
  configuration: LintConfig,
  filters: ActiveFilters,
): Promise<string> {
  const sortedFiles = sortedByPosition(files);
  const triggeredRules = new Set<string>();
  for (const file of sortedFiles) {
    for (const finding of file.findings) {
      if (finding.rule) {
        triggeredRules.add(finding.rule);
      }
    }
  }
  const ruleDetails = [...triggeredRules]
    .sort((a, b) => a.localeCompare(b))
    .map((rule) => ruleTagsByRule.get(rule))
    .filter((info): info is RuleTagsInfo => info !== undefined)
    .map(({ rule, description, kinds, importance, auto_fixable, doc_url }) => ({
      rule,
      description,
      kinds,
      importance,
      auto_fixable,
      doc_url,
    }));

  // `level` carries the severity separately, so avoid repeating its internal
  // message prefix in the AI-focused representation.
  const baseReport = buildIssuesReport(sortedFiles, true);
  const findings = {
    files: await Promise.all(
      baseReport.files.map(async (fileReport, fileIndex) => ({
        ...fileReport,
        severity_counts: severityCounts(fileReport.diagnostics),
        rule_counts: ruleCounts(fileReport.diagnostics),
        source: sources.get(fileReport.path) ?? null,
        diagnostics: await Promise.all(
          fileReport.diagnostics.map(async (diagnostic, diagnosticIndex) => {
            const finding = sortedFiles[fileIndex].findings[diagnosticIndex];
            const tagged =
              diagnostic.rule === COMPILER_ERROR_RULE
                ? { ...diagnostic, external: true, source: "compiler" }
                : diagnostic;
            if (!finding.rule || !FIXABLE_RULE_IDS.has(finding.rule) || hasNoAutomaticFix(finding)) {
              return tagged;
            }
            const repair = await previewRepairPscLine(sortedFiles[fileIndex].path, finding.rule, finding.line);
            return repair === null ? tagged : { ...tagged, repair };
          }),
        ),
      })),
    ),
    total_diagnostics: baseReport.total_diagnostics,
    severity_counts: severityCounts(baseReport.files.flatMap((file) => file.diagnostics)),
    rule_counts: ruleCounts(baseReport.files.flatMap((file) => file.diagnostics)),
  };

  return JSON.stringify(
    {
      $schema: AI_EXPORT_SCHEMA_URL,
      header: {
        tool: TOOL_NAME,
        version: version || "unknown",
        website: WEBSITE_URL,
        target_game: TARGET_GAME,
        generated_at: new Date().toISOString(),
      },
      configuration: aiConfiguration(configuration),
      filters,
      findings,
      rule_details: ruleDetails,
    },
    null,
    2,
  );
}

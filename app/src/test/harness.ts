import { afterEach, beforeEach, expect, vi } from "vitest";
import { invokeMock } from "./mocks";
import { mountFixture } from "./fixture";
import { cancelLiveEditLint } from "../live-edit-lint";
import { loadProjectConfig, resetConfirmedProjectDirs } from "../project-settings";
import { dirnameOf } from "../path";
import { stopWatchMode } from "../watch";
import { type RuleTagsInfo } from "../backend-types";
import { DEFAULT_LINT_CONFIG, type LintConfig } from "../config-types";
import { ruleTagsByRule } from "../results-filter";
import { aiConfiguration } from "../results-export-ai";

// Default backend behavior for the project-root discovery commands (see
// project-io.ts's projectDirForAchlist/projectDirForDirectory/
// projectDirForPscPath/loadProjectInfo), for tests that drive
// handleDroppedPaths without caring about the exact root a particular drop
// resolves to: just the naive fallback each of those functions itself would
// use if the real (Rust) `scripts/source`/`source/scripts`-pair lookup found
// nothing. A test asserting a specific resolved root (e.g. one where the
// achlist doesn't live in the project root itself) still needs its own
// explicit `find_project_root`/`find_psc_project_root_for_path` handler.
function defaultProjectRootHandler(command: string): ((args: unknown) => unknown) | undefined {
  switch (command) {
    case "load_lookup_script_roots":
      return () => [];
    case "load_script_roots":
      return () => [];
    case "load_compiler_path":
      return () => "";
    case "load_compile_check":
      return () => false;
    case "load_project_info":
      return () => ({ detected_script_roots: [], used_configuration_file: null });
    case "save_compiler_path":
    case "save_compile_check":
    case "save_script_roots":
    case "save_lookup_script_roots":
      return () => undefined;
    case "find_project_root":
      return (args) => (args as { fallback: string }).fallback;
    case "find_psc_project_root_for_path":
      return (args) => dirnameOf(dirnameOf(dirnameOf((args as { path: string }).path)));
    default:
      return undefined;
  }
}

// Default backend behavior for the startup metadata commands that
// main.ts fires on DOMContentLoaded when isTauri() is true (see
// loadAppVersion/loadRuleTags/refreshPresetManagementTab). Tests that
// import the harness and therefore load main.ts hit these even when they
// never call invokeImplFor themselves; rejecting them would just
// console.error from the wrappers. A test that cares about the returned
// version/tags/presets still supplies its own handlers.
function defaultStartupHandler(command: string): ((args: unknown) => unknown) | undefined {
  switch (command) {
    case "get_app_version":
      return () => "";
    case "list_rule_tags":
      return () => [];
    case "list_config_presets":
      return () => [];
    default:
      return undefined;
  }
}

// Default backend behavior for config load/save and other best-effort
// commands whose wrappers catch a failed invoke and console.error it
// (see config-io.ts and previewRepairPscLine). Tests that assert a
// specific persist or preview still supply their own handlers.
function defaultBestEffortHandler(command: string): ((args: unknown) => unknown) | undefined {
  switch (command) {
    case "load_lint_config":
    case "load_lint_config_from_path":
      return () => DEFAULT_LINT_CONFIG;
    case "save_lint_config":
    case "save_lint_config_to_path":
      return () => undefined;
    case "preview_repair_psc_line":
      return () => null;
    default:
      return undefined;
  }
}

// Test-only stand-ins for the format_issues_as_text/format_issues_as_json/
// format_issues_for_ai_base Tauri commands (app/src-tauri/src/export.rs),
// which vitest can't invoke for real since it never runs the Rust
// papyrus-lint-output crate they're built on. Rather than duplicate that
// crate's own formatting logic here, these reuse `ruleTagsByRule` (the
// same rule-tag cache the real commands' doc_url/rule_details would be
// built from, via papyrus_lints::tags - see list_rule_tags/applyRuleTags)
// and aiConfiguration (still kept in results-export-ai.ts as a small pure
// helper, mirroring the Rust crate's own ai_configuration) so a test that
// populates ruleTagsByRule via applyRuleTags sees the same doc_url/
// rule_details the real backend would compute for those rules.
interface FakeDiagnosticInput {
  line: number;
  column: number;
  rule: string;
  message: string;
}

interface FakeIssuesFileInput {
  path: string;
  findings: FakeDiagnosticInput[];
}

function fakeLevelOf(message: string): "error" | "warning" | "info" {
  if (message.startsWith("[warning]")) {
    return "warning";
  }
  if (message.startsWith("[info]")) {
    return "info";
  }
  return "error";
}

function fakeStripSeverityPrefix(message: string): string {
  return message.replace(/^\[(?:error|warning|info)\]\s*/, "");
}

function fakeDocUrlFor(rule: string): string | null {
  return ruleTagsByRule.get(rule)?.doc_url ?? null;
}

function fakeJsonDiagnostic(finding: FakeDiagnosticInput, stripSeverityPrefix: boolean) {
  return {
    line: finding.line,
    column: finding.column,
    rule: finding.rule,
    level: fakeLevelOf(finding.message),
    message: stripSeverityPrefix ? fakeStripSeverityPrefix(finding.message) : finding.message,
    doc_url: fakeDocUrlFor(finding.rule),
  };
}

function fakeSeverityCounts(diagnostics: { level: string }[]) {
  return {
    errors: diagnostics.filter((d) => d.level === "error").length,
    warnings: diagnostics.filter((d) => d.level === "warning").length,
    info: diagnostics.filter((d) => d.level === "info").length,
  };
}

function fakeRuleCounts(diagnostics: { rule: string }[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const diagnostic of diagnostics) {
    counts[diagnostic.rule] = (counts[diagnostic.rule] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([a], [b]) => a.localeCompare(b)));
}

function fakeFormatIssuesAsText(args: unknown): string {
  const { files } = args as { files: FakeIssuesFileInput[] };
  const lines: string[] = [];
  for (const file of files) {
    for (const finding of file.findings) {
      const docUrl = fakeDocUrlFor(finding.rule);
      const suffix = docUrl ? ` (${docUrl})` : "";
      lines.push(`${file.path}:${finding.line}:${finding.column}: [${finding.rule}] ${finding.message}${suffix}`);
    }
  }
  return lines.join("\n");
}

function fakeFormatIssuesAsJson(args: unknown): string {
  const { files } = args as { files: FakeIssuesFileInput[] };
  let totalDiagnostics = 0;
  const jsonFiles = files.map((file) => {
    totalDiagnostics += file.findings.length;
    return {
      path: file.path,
      diagnostics: file.findings.map((finding) => fakeJsonDiagnostic(finding, false)),
      parser_errors: [],
      diff: null,
    };
  });
  return JSON.stringify({
    files: jsonFiles,
    files_with_diagnostics: jsonFiles.length,
    total_diagnostics: totalDiagnostics,
  });
}

function fakeFormatIssuesForAiBase(args: unknown): string {
  const { files, configuration, version } = args as {
    files: (FakeIssuesFileInput & { source: unknown })[];
    configuration: LintConfig;
    version: string;
  };
  const aiFiles = files.map((file) => {
    const diagnostics = file.findings.map((finding) => fakeJsonDiagnostic(finding, true));
    return {
      path: file.path,
      severity_counts: fakeSeverityCounts(diagnostics),
      rule_counts: fakeRuleCounts(diagnostics),
      diagnostics,
      parser_errors: [],
      source: file.source ?? null,
    };
  });
  const allDiagnostics = aiFiles.flatMap((file) => file.diagnostics);
  const triggeredRules = [...new Set(allDiagnostics.map((diagnostic) => diagnostic.rule))].sort((a, b) =>
    a.localeCompare(b),
  );
  const ruleDetails = triggeredRules
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

  return JSON.stringify({
    $schema: "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v4.schema.json",
    header: {
      tool: "Papyrus Lint",
      version,
      website: "https://papyrus-lint.idrinth.de",
      target_game: "Skyrim SE/AE",
      generated_at: new Date().toISOString(),
    },
    configuration: aiConfiguration(configuration),
    findings: {
      files: aiFiles,
      total_diagnostics: allDiagnostics.length,
      severity_counts: fakeSeverityCounts(allDiagnostics),
      rule_counts: fakeRuleCounts(allDiagnostics),
    },
    rule_details: ruleDetails,
  });
}

function defaultExportHandler(command: string): ((args: unknown) => unknown) | undefined {
  switch (command) {
    case "format_issues_as_text":
      return fakeFormatIssuesAsText;
    case "format_issues_as_json":
      return fakeFormatIssuesAsJson;
    case "format_issues_for_ai_base":
      return fakeFormatIssuesForAiBase;
    default:
      return undefined;
  }
}

function defaultCompletionHandler(command: string): ((args: unknown) => unknown) | undefined {
  if (command !== "resolve_completion_query") {
    return undefined;
  }
  return (args: unknown) => {
    const { source, cursorIndex } = args as { source: string; cursorIndex: number };
    const match = /([A-Za-z_]\w*)(?:\s*\[[^[\]]*\])?\s*\.(\w*)$/.exec(source.slice(0, cursorIndex));
    if (!match) {
      return null;
    }
    const [, receiver, prefix] = match;
    const script = /^\s*ScriptName\s+(\w+)(?:\s+Extends\s+(\w+))?/im.exec(source);
    let receiverType = receiver.toLowerCase() === "self" ? script?.[1] : receiver.toLowerCase() === "parent" ? script?.[2] : undefined;
    if (!receiverType) {
      const declaration = new RegExp(`^\\s*(\\w+)(?:\\[\\])?\\s+(?:Property\\s+)?${receiver}\\b`, "im").exec(source);
      receiverType = declaration?.[1];
    }
    return receiverType ? { receiverType, prefix, prefixStart: cursorIndex - prefix.length } : null;
  };
}

// Stand-in for `lint_project_scripts` when a test still describes the batch
// in the old per-file commands. The frontend only invokes the batch command;
// this drives `parse_psc_file` / `lint_psc_file` / `preload_project_scripts`
// handlers directly (not through `invoke`) and streams the same events the
// real command would, so a pending lint still updates the list one file at
// a time. A test that supplies its own `lint_project_scripts` handler skips
// this.
async function emulateLintProjectScripts(
  handlers: Record<string, (args: unknown) => unknown>,
  args: unknown,
): Promise<{ path: string; ok: boolean; detail: string; findings: unknown[] }[]> {
  const { paths, context, onEvent } = args as {
    paths: string[];
    context: { config?: { game?: string } };
    onEvent?: { onmessage: (event: unknown) => void };
  };
  const emit = (event: unknown) => onEvent?.onmessage?.(event);
  if (handlers.preload_project_scripts) {
    await handlers.preload_project_scripts({
      paths,
      context,
      onProgress: {
        onmessage: (progress: { phase: string; completed: number; total: number }) => {
          emit({ kind: "progress", ...progress });
        },
      },
    });
  }
  const outcomes: { path: string; ok: boolean; detail: string; findings: unknown[] }[] = [];
  const parse = handlers.parse_psc_file;
  const lint = handlers.lint_psc_file;
  const game = context?.config?.game;
  for (let index = 0; index < paths.length; index++) {
    const path = paths[index];
    let outcome: { path: string; ok: boolean; detail: string; findings: unknown[] };
    try {
      if (!parse || !lint) {
        outcome = { path, ok: true, detail: 'parsed as ""', findings: [] };
      } else {
        const script = (await parse({ path, game })) as { name: string };
        const findings = (await lint({ path, context })) as unknown[];
        outcome = { path, ok: true, detail: `parsed as "${script.name}"`, findings };
      }
    } catch (error) {
      outcome = { path, ok: false, detail: String(error), findings: [] };
    }
    outcomes.push(outcome);
    emit({ kind: "result", ...outcome });
    emit({ kind: "progress", phase: "Linting", completed: index + 1, total: paths.length });
  }
  return outcomes;
}

export function invokeImplFor(handlers: Record<string, (args: unknown) => unknown>) {
  invokeMock.mockImplementation((command: string, args: unknown) => {
    if (command === "lint_project_scripts" && !handlers[command]) {
      return emulateLintProjectScripts(handlers, args);
    }
    const handler = handlers[command] ?? defaultProjectRootHandler(command) ?? defaultExportHandler(command) ?? defaultCompletionHandler(command) ?? defaultStartupHandler(command) ?? defaultBestEffortHandler(command);
    if (!handler) {
      return Promise.reject(new Error(`unexpected command: ${command}`));
    }
    return Promise.resolve(handler(args));
  });
}

// Waits for the "select this project's configuration" dialog
// (promptForConfigSelection) to open and clicks "Continue", accepting
// useProjectDir's own auto-detection either way (an existing configuration
// file, or the engine's silent defaults if the project has none). Pumps
// microtasks directly instead of vi.waitFor, so it works the same whether
// or not a test has switched to fake timers.
export async function confirmDetectedConfig(): Promise<void> {
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  expect(picker.hasAttribute("open")).toBe(true);
  document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
}

// Drives useProjectDir through loadProjectConfig's own "select this
// project's configuration" step for tests that don't care about that step
// itself, immediately accepting whatever useProjectDir would already do on
// its own (see confirmDetectedConfig). A directory already confirmed this
// session (see resetConfirmedProjectDirs) skips the dialog entirely, the
// same as loadProjectConfig itself does.
export async function loadProjectConfigConfirmed(dir: string): Promise<void> {
  const pending = loadProjectConfig(dir);
  const picker = document.querySelector<HTMLDialogElement>("#config-picker")!;
  for (let i = 0; i < 30 && !picker.hasAttribute("open"); i++) {
    await Promise.resolve();
  }
  if (picker.hasAttribute("open")) {
    document.querySelector<HTMLButtonElement>("#config-picker-continue")!.click();
  }
  await pending;
}

beforeEach(() => {
  invokeMock.mockReset();
  // Installs the default project-root/export-formatting/startup/best-effort
  // fallbacks (see defaultProjectRootHandler/defaultExportHandler/
  // defaultStartupHandler/defaultBestEffortHandler above) so a test that
  // never calls invokeImplFor itself still gets sensible behavior for
  // those commands; a test that does call invokeImplFor merges its own
  // handlers back on top of this same default chain.
  invokeImplFor({});
  localStorage.clear();
  mountFixture();
  resetConfirmedProjectDirs();
  // A previous test's live-lint debounce timer (see scheduleLiveEditLint in
  // live-edit.ts) would otherwise fire against this test's fresh DOM/mocks once
  // its delay elapses, so it's cancelled up front the same way
  // resetConfirmedProjectDirs above resets other module-level state.
  cancelLiveEditLint();
  // Likewise for a previous test's watch-mode poll interval (see
  // startWatchMode in watch.ts), which would otherwise keep firing against
  // this test's fresh DOM/mocks.
  stopWatchMode();
});

afterEach(() => {
  vi.restoreAllMocks();
  cancelLiveEditLint();
  stopWatchMode();
});

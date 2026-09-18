// Thin wrappers around every Tauri `invoke()` call the frontend makes for
// linting, repairing, compiling, and looking up script members, plus the
// wire types they exchange with the Rust backend. Kept separate from the
// orchestration in main.ts so that layer isn't tangled up with how each
// individual command is dispatched.
import { invoke } from "@tauri-apps/api/core";
import { type Member } from "./autocomplete";
import { currentLintConfig, type LintConfig } from "./config";
import {
  currentCompileCheck,
  currentCompilerPath,
  currentLookupScriptRoots,
  currentProjectDir,
  effectiveScriptRoots,
} from "./project";

export interface PapyrusScript {
  name: string;
}

export interface Diagnostic {
  line: number;
  column: number;
  message: string;
  // The lint rule that raised this finding (e.g. "trailing-whitespace"),
  // matching papyrus_lints::Diagnostic::rule. Optional here since not every
  // test fixture needs one; the backend always sends it.
  rule?: string;
}

// Appends the triggered rule id in parentheses at the end of a finding's
// on-screen message. A finding with no rule id (typical of a test fixture)
// is labelled `(unknown)`.
export function findingMessageWithRule(finding: Diagnostic): string {
  return `${finding.message} (${finding.rule ?? "unknown"})`;
}

export interface PscParseOutcome {
  path: string;
  ok: boolean;
  detail: string;
  findings: Diagnostic[];
}

// Mirrors papyrus_lints::tags::Importance's lowercase serde rename.
export type TagImportance = "low" | "medium" | "high";
export const TAG_IMPORTANCES: TagImportance[] = ["low", "medium", "high"];

// The kind keyword(s) papyrus_lints::tags currently tags every rule with.
// Kept in sync by hand with the "kinds" used across RULE_TAGS in
// app/crates/papyrus-lints/src/tags.rs, the same convention FIXABLE_RULE_IDS
// below follows.
export type TagKind = "style" | "performance" | "correctness" | "maintainability";
export const TAG_KINDS: TagKind[] = ["style", "performance", "correctness", "maintainability"];

// One rule's tag metadata, as returned by the backend's list_rule_tags
// command (papyrus_lints::tags::RuleTags, made JSON-friendly).
export interface RuleTagsInfo {
  rule: string;
  // The rule's detailed description, copied from its row in README.md's
  // Implemented Lints tables (see papyrus_lints::tags::RuleTags).
  description: string;
  kinds: string[];
  importance: TagImportance;
  auto_fixable: boolean;
  // This rule's own documentation link (papyrus_lints::tags::RuleTags::doc_url),
  // for linking a finding straight to its explanation on the project website.
  doc_url: string;
}

export interface CompileOutcome {
  success: boolean;
  stdout: string;
  stderr: string;
  personal_data_stripped: boolean;
}

// Mirrors the backend's ProjectLintContext: the project-level inputs shared
// by lint_psc_file and the mutating repair commands. Built once here so a
// new project-level option only has to be added in one frontend helper
// rather than every invoke() payload independently.
export interface ProjectLintContext {
  root: string;
  config: LintConfig;
  additional_roots: string[];
  lookup_roots: string[];
  compiler_path: string;
  compile_check: boolean;
}

export function currentProjectLintContext(): ProjectLintContext {
  return {
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additional_roots: effectiveScriptRoots(),
    lookup_roots: currentLookupScriptRoots,
    compiler_path: currentCompilerPath,
    compile_check: currentCompileCheck,
  };
}

// Lints `source` directly, in-process (the same `lint_papyrus_script`
// Tauri command `app/src-tauri/src/files.rs` wraps around
// `papyrus_lints::lint`), instead of a `.psc` path on disk. Used by the
// code viewer's edit mode for live, as-you-type feedback on the textarea's
// current (possibly unsaved) contents - see `scheduleLiveEditLint` in
// live-edit.ts. Unlike `lintPscFile`, this never resolves cross-script
// lookups (there's no project root to resolve them against), the same
// tradeoff the CLI's own `--blob` flag makes for editor extensions that
// only have the buffer's text in memory.
export async function lintPapyrusScript(source: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_papyrus_script", { source, config: currentLintConfig });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export async function lintPscFile(path: string): Promise<Diagnostic[]> {
  try {
    return await invoke<Diagnostic[]>("lint_psc_file", {
      path,
      context: currentProjectLintContext(),
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

export async function repairPscFile(path: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file", {
    path,
    context: currentProjectLintContext(),
  });
}

// Computes the same whole-file automatic fix repairPscFile would apply, but
// never writes it to disk: returns a standard unified diff of what would
// change (or an empty string if nothing would), the same output
// `PapyrusLinterCLI fix --dry-run` prints. Drives the code viewer's
// "Preview fixes" button.
export async function previewRepairPscFile(path: string): Promise<string> {
  return invoke<string>("preview_repair_psc_file", {
    path,
    config: currentLintConfig,
  });
}

// Computes just `rule`'s automatic fix and returns what `line` (1-indexed)
// would look like afterward, without writing anything to disk or touching
// any other line - `null` when there's nothing meaningful to show (the fix
// doesn't change the file, would shift the line count elsewhere, or simply
// doesn't touch `line`). Drives formatIssuesForAi's per-finding `repair`
// preview in results-export-ai.ts.
export async function previewRepairPscLine(path: string, rule: string, line: number): Promise<string | null> {
  try {
    return await invoke<string | null>("preview_repair_psc_line", {
      path,
      config: currentLintConfig,
      rule,
      line,
    });
  } catch (error) {
    console.error(error);
    return null;
  }
}

// Rule ids with an automatic fix (papyrus_lints::FIXABLE_RULE_IDS), used to
// decide which findings offer the per-finding "Fix this issue" button. Kept
// in sync by hand with FIXABLE_RULE_IDS in
// app/crates/papyrus-lints/src/lib.rs.
export const FIXABLE_RULE_IDS = new Set([
  "identifier-casing",
  "slow-functions",
  "semicolon",
  "indentation",
  "property-sorting",
  "comma-spacing",
  "chain-whitespace",
  "exclamation-spacing",
  "operator-spacing",
  "assignment-operator-spacing",
  "type-casing",
  "trailing-whitespace",
  "global-variable-increment",
  "unnecessary-function",
  "unused-import",
]);

// A rule in FIXABLE_RULE_IDS can still report a violation it can't actually
// repair without a substantive rename (e.g. type-casing on a name with
// underscores, such as a compiler-generated fragment script's ScriptName) --
// see papyrus_lints::type_casing::check, which appends this same note to
// such a finding's own message rather than letting a caller assume every
// finding from a "fixable" rule can be fixed.
const NO_AUTOMATIC_FIX_NOTE = "no automatic fix";

export function hasNoAutomaticFix(finding: Diagnostic): boolean {
  return finding.message.includes(NO_AUTOMATIC_FIX_NOTE);
}

export function isFixableFinding(finding: Diagnostic): boolean {
  return finding.rule !== undefined && FIXABLE_RULE_IDS.has(finding.rule) && !hasNoAutomaticFix(finding);
}

export function hasFixableFindings(findings: Diagnostic[]): boolean {
  return findings.some((finding) => isFixableFinding(finding));
}

// Applies just `rule`'s own automatic fix, restricted to `line` (see
// repair_psc_finding/papyrus_lints::restrict_to_line on the backend),
// leaving every other line and finding untouched. Rejects (e.g. a fix that
// would change the file's line count elsewhere, like property-sorting
// relocating a declaration) rather than falling back to the whole-file
// "Apply fixes" behavior, so the caller can surface why this one issue
// couldn't be fixed on its own.
export async function repairPscFinding(path: string, rule: string, line: number): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_finding", {
    path,
    context: currentProjectLintContext(),
    rule,
    line,
  });
}

// Applies just `rule`'s own automatic fix across the whole of `path`, like
// repairPscFinding but without restricting it to a single line — the "mass
// fix" action calls this once per file to clear every occurrence of one
// issue (e.g. every trailing-whitespace finding) project-wide in one go.
export async function repairPscFileRule(path: string, rule: string): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("repair_psc_file_rule", {
    path,
    context: currentProjectLintContext(),
    rule,
  });
}

// Adds (or extends) an `; @disable <rules>` comment on `line`, silencing
// every named rule there instead of fixing it — the code viewer's per-line
// "Ignore" button, the inverse of repairPscFinding's per-line "Fix" button.
// See add_disable_comment_to_psc_line/papyrus_lints::add_disable_comment on
// the backend for the exact merging rules.
export async function addDisableCommentToPscLine(path: string, rules: string[], line: number): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("add_disable_comment_to_psc_line", {
    path,
    context: currentProjectLintContext(),
    rules,
    line,
  });
}

export async function writePscFile(path: string, contents: string): Promise<void> {
  await invoke("write_psc_file", { path, contents });
}

// Returns each existing path in `paths`' own last-modified time, as Unix
// milliseconds, keyed by that same path — a path that no longer exists or
// can't be read is simply absent from the result. Used by watch mode
// (`watch.ts`) to poll for changes to the currently loaded `.psc` files.
// Returns an empty map on failure, the same fallback every other
// best-effort lookup here uses, rather than surfacing an error for what's
// just a periodic background poll.
export async function getPscFileMtimes(paths: string[]): Promise<Record<string, number>> {
  try {
    return await invoke<Record<string, number>>("get_psc_file_mtimes", { paths });
  } catch (error) {
    console.error(error);
    return {};
  }
}

// Fetches every function/property available on an object of type
// `typeName` (including those inherited via Extends), for the code
// viewer's `.`-triggered autocompletion. `root` is the project root (see
// projectDirForAchlist/projectDirForPscPath in project.ts), the same as
// every other command that resolves scripts across a project.
export async function listScriptMembers(typeName: string): Promise<Member[]> {
  try {
    return await invoke<Member[]>("list_script_members", {
      root: currentProjectDir ?? "",
      typeName,
      additionalRoots: effectiveScriptRoots(),
      lookupRoots: currentLookupScriptRoots,
    });
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Compiles the `.psc` file at `path` with the currently configured
// PapyrusCompiler.exe path, reproducing the invocation Creation Kit
// tooling uses to compile a single script out of its source directory.
export async function compilePscFile(path: string): Promise<CompileOutcome> {
  return invoke<CompileOutcome>("compile_psc_file", {
    path,
    compilerPath: currentCompilerPath,
    additionalRoots: effectiveScriptRoots(),
  });
}

// Fetches every built-in lint rule's tag metadata (kind(s), importance, and
// whether it's auto-fixable; see papyrus_lints::tags) from the Rust
// backend, for grouping/filtering the lint results by tag. Returns an
// empty array if the lookup fails.
export async function loadRuleTags(): Promise<RuleTagsInfo[]> {
  try {
    return (await invoke<RuleTagsInfo[]>("list_rule_tags")) ?? [];
  } catch (error) {
    console.error(error);
    return [];
  }
}

// Fetches the desktop app's version from the Rust backend, so it can be
// shown to the user. Returns an empty string if the lookup fails.
export async function loadAppVersion(): Promise<string> {
  try {
    return await invoke<string>("get_app_version");
  } catch (error) {
    console.error(error);
    return "";
  }
}

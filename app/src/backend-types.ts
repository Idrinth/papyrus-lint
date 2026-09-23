import { type LintConfig } from "./config-types";

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
export type TagKind = "style" | "performance" | "correctness" | "maintainability";
export const TAG_KINDS: TagKind[] = ["style", "performance", "correctness", "maintainability"];

// One rule's tag metadata, as returned by the backend's list_rule_tags command.
export interface RuleTagsInfo {
  rule: string;
  description: string;
  kinds: string[];
  importance: TagImportance;
  auto_fixable: boolean;
  doc_url: string;
}

export interface CompileOutcome {
  success: boolean;
  stdout: string;
  stderr: string;
  personal_data_stripped: boolean;
}

export interface ProjectLintContext {
  root: string;
  config: LintConfig;
  additional_roots: string[];
  lookup_roots: string[];
  compiler_path: string;
  compile_check: boolean;
}

import { type LintConfig } from "./config-types";

export interface TypeNameRef {
  name: string;
  is_array: boolean;
}

export interface ParamRef {
  name: string;
  type_name: TypeNameRef;
}

export interface FunctionMember {
  kind: "function";
  name: string;
  params: ParamRef[];
  return_type: TypeNameRef | null;
  is_global: boolean;
  is_native: boolean;
  is_event: boolean;
  // Inner text of the `{ ... }` documentation comment after this function's
  // header, when the backend (or a local overlay of the buffer being edited)
  // found one. Absent/null/empty means there's nothing to show.
  doc?: string | null;
}

export interface PropertyMember {
  kind: "property";
  name: string;
  type_name: TypeNameRef;
  // Inner text of the `{ ... }` documentation comment after this property's
  // header; same rules as [`FunctionMember.doc`].
  doc?: string | null;
}

// Mirrors `papyrus_lint_core::function_table::Member`, as returned by the
// `list_script_members` Tauri command.
export type Member = FunctionMember | PropertyMember;

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

export interface ProjectInfo {
  detected_script_roots: string[];
  used_configuration_file: string | null;
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
  strict_achlist_scope: boolean;
  known_scripts: string[];
}

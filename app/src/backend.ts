// Thin wrappers around every Tauri `invoke()` call the frontend makes for
// linting, repairing, compiling, and looking up script members, plus the
// wire types they exchange with the Rust backend. Kept separate from the
// orchestration in main.ts so that layer isn't tangled up with how each
// individual command is dispatched.
import { invoke } from "@tauri-apps/api/core";
import { type Member } from "./autocomplete";
import { currentLintConfig } from "./config";
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

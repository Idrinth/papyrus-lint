import { type ProjectLintContext } from "./backend-types";
import { currentLintConfig } from "./config-types";
import {
  currentCompileCheck,
  currentCompilerPath,
  currentLookupScriptRoots,
  currentProjectDir,
  currentProjectScripts,
  currentStrictAchlistScope,
  effectiveScriptRoots,
} from "./project-state";

// Builds the project-level inputs shared by lint and repair commands in one
// place so new options do not have to be added to every invoke payload.
export function currentProjectLintContext(): ProjectLintContext {
  return {
    root: currentProjectDir ?? "",
    config: currentLintConfig,
    additional_roots: effectiveScriptRoots(),
    lookup_roots: currentLookupScriptRoots,
    compiler_path: currentCompilerPath,
    compile_check: currentCompileCheck,
    strict_achlist_scope: currentStrictAchlistScope,
    known_scripts: currentProjectScripts,
  };
}

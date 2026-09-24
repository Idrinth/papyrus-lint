// Thin wrappers around every Tauri `invoke()` call the frontend makes for
// linting, repairing, compiling, and looking up script members. Kept separate
// from the orchestration in main.ts so that layer isn't tangled up with how
// each individual command is dispatched.
import { invoke } from "@tauri-apps/api/core";
import * as tauriCore from "@tauri-apps/api/core";
import type { Member } from "./autocomplete-types";
import { currentProjectLintContext } from "./backend-context";
import { type CompileOutcome, type Diagnostic, type RuleTagsInfo } from "./backend-types";
import { currentLintConfig } from "./config-types";
import { currentCompilerPath, currentLookupScriptRoots, currentProjectDir, effectiveScriptRoots } from "./project-state";
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

// Parses every one of `paths` up front and preloads the current project's
// shared function table from the result (see `preload_project_scripts` in
// `app/src-tauri/src/lint.rs`), before `parsePscFiles` (drop.ts) runs its own
// per-file `parse_psc_file`/`lintPscFile` pair across the same batch.
// Resolving an Extends/type reference to another script in `paths` is then
// a cache hit from the start for every one of those per-file calls, rather
// than a write-locked, on-demand parse the first one to need it triggers.
// Purely a perf optimization -- a failure here is logged and otherwise
// ignored, since the per-file calls that follow still resolve everything
// correctly (just without this head start) either way.
//
// `onProgress` receives the same counts the CLI's "Parsing" bar prints
// while the type closure walks referenced scripts (`phase` is "Resolving"
// with a growing `total`), then one indeterminate "Indexing scripts"
// update (`total` 0) while those parses are merged into the function
// table. The channel is optional so a caller that cannot construct one
// (unit tests mock this module without `Channel`) still preloads.
export interface PreloadProgress {
  phase: string;
  completed: number;
  total: number;
}

// `Channel` is a real export of `@tauri-apps/api/core`, but the unit-test
// mocks of that module usually only stub `invoke`. A namespace import
// stays synchronous (an `await import()` here shifts the lint loop by a
// microtask and breaks tests that drain a fixed number of turns) and is
// simply missing on those mocks.
function preloadProgressChannel(onProgress?: (progress: PreloadProgress) => void) {
  try {
    const core = tauriCore as { Channel?: new () => { onmessage: ((progress: PreloadProgress) => void) | null } };
    if (typeof core.Channel !== "function") {
      return undefined;
    }
    const channel = new core.Channel();
    channel.onmessage = (progress) => onProgress?.(progress);
    return channel;
  } catch {
    // Vitest's mock of this module throws on any export it didn't stub,
    // including `Channel`. Preload still runs; it just can't stream progress.
    return undefined;
  }
}

export async function preloadProjectScripts(
  paths: string[],
  onProgress?: (progress: PreloadProgress) => void,
): Promise<void> {
  const onProgressChannel = preloadProgressChannel(onProgress);
  try {
    await invoke("preload_project_scripts", {
      paths,
      context: currentProjectLintContext(),
      ...(onProgressChannel ? { onProgress: onProgressChannel } : {}),
    });
  } catch (error) {
    console.error(error);
  } finally {
    // Drop the handler so a finished run doesn't keep the progress closure,
    // and the webview callback it registered, alive across later lints.
    if (onProgressChannel) {
      onProgressChannel.onmessage = () => {};
    }
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

// Adds (or extends) an `; @disable-file <rules>` comment on `line`, silencing
// every named rule across the whole file instead of just that line — the
// code viewer's per-line "File disable" button. See
// add_disable_file_comment_to_psc_line/papyrus_lints::add_disable_file_comment
// on the backend for the exact merging rules.
export async function addDisableFileCommentToPscLine(
  path: string,
  rules: string[],
  line: number,
): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("add_disable_file_comment_to_psc_line", {
    path,
    context: currentProjectLintContext(),
    rules,
    line,
  });
}

// Adds `; @nodiscard` to `line`'s function header (or extends its existing
// trailing comment) - the code viewer's per-line "Nodiscard" button,
// offered only where nodiscardEligibleLines (nodiscard.ts) says the header
// is eligible: a function that returns a value or is Native, and isn't
// flagged already. See add_nodiscard_comment_to_psc_line/
// papyrus_lints::add_nodiscard_comment on the backend for the exact
// merging rules.
export async function addNodiscardCommentToPscLine(path: string, line: number): Promise<Diagnostic[]> {
  return invoke<Diagnostic[]>("add_nodiscard_comment_to_psc_line", {
    path,
    context: currentProjectLintContext(),
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
// projectDirForAchlist/projectDirForPscPath in project-io.ts), the same as
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

export interface CompletionQuery {
  receiverType: string;
  prefix: string;
  prefixStart: number;
}

// Resolves the declared type on the left of the member-access dot in Rust,
// where source-language analysis belongs. The frontend keeps only dropdown
// rendering and insertion concerns.
export async function resolveCompletionQuery(source: string, cursorIndex: number): Promise<CompletionQuery | null> {
  try {
    return await invoke<CompletionQuery | null>("resolve_completion_query", { source, cursorIndex });
  } catch (error) {
    console.error(error);
    return null;
  }
}

// Compiles the `.psc` file at `path` with the currently configured
// PapyrusCompiler.exe path, reproducing the invocation Creation Kit
// tooling uses to compile a single script out of its source directory.
export async function compilePscFile(path: string): Promise<CompileOutcome> {
  return invoke<CompileOutcome>("compile_psc_file", {
    path,
    game: currentLintConfig.game,
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

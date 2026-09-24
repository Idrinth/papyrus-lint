//! Project-aware linting, compile-check, and compile commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{
    ast_cache, compile_diagnostics, compiler, function_table, script_locator, stale_pex,
};
use papyrus_lints::script_filename_mismatch;
use serde::{Deserialize, Serialize};

/// Identity of one desktop-app [`function_table::FunctionTable`]: project
/// root plus the two configured search-root lists. Concurrent Tauri
/// commands for the same project reuse one `RwLock`-guarded table through
/// [`function_table::SharedFunctionTable`], matching the CLI's `--threads`
/// workers rather than building an empty table per file. The exclusive lock
/// is taken only when a lookup still has to parse a script the parse-phase
/// type closure did not already preload.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SharedTableKey {
    game: papyrus_lints::Game,
    root: PathBuf,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
}

fn shared_tables(
) -> &'static Mutex<HashMap<SharedTableKey, Arc<RwLock<function_table::FunctionTable>>>> {
    static TABLES: OnceLock<
        Mutex<HashMap<SharedTableKey, Arc<RwLock<function_table::FunctionTable>>>>,
    > = OnceLock::new();
    TABLES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn project_function_table(
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Arc<RwLock<function_table::FunctionTable>> {
    project_function_table_for_game(
        papyrus_lints::Game::default(),
        root,
        additional_roots,
        lookup_roots,
    )
}

pub(crate) fn project_function_table_for_game(
    game: papyrus_lints::Game,
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Arc<RwLock<function_table::FunctionTable>> {
    let key = SharedTableKey {
        game,
        root: PathBuf::from(&root),
        additional_roots: additional_roots.clone(),
        lookup_roots: lookup_roots.clone(),
    };
    let mut cache = shared_tables()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .entry(key)
        .or_insert_with(|| {
            Arc::new(RwLock::new(
                function_table::FunctionTable::new_with_additional_roots(
                    PathBuf::from(root),
                    additional_roots,
                )
                .with_game(game)
                .with_lookup_roots(lookup_roots),
            ))
        })
        .clone()
}

/// Project-level inputs shared by [`lint_psc_file`] and the mutating repair
/// commands: the project root, lint configuration, extra script roots,
/// analysis-only lookup roots, compiler path, and whether to merge
/// PapyrusCompiler.exe errors into the result. Grouped so adding a
/// project-level option only changes this type (and the frontend helper that
/// builds it) rather than every command contract independently.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ProjectLintContext {
    /// The project root (conventionally the directory containing the
    /// `.achlist` file); it lets the "Argument type check" lint resolve
    /// calls to functions declared on other scripts under `root`, and lets
    /// the "Return type check" lint accept a returned value whose script
    /// under `root` extends the declared return type.
    pub(crate) root: String,
    pub(crate) config: papyrus_lints::Config,
    /// The project's configured additional script roots (see
    /// [`load_script_roots`]), searched the same way alongside `root`'s
    /// conventional source directories.
    pub(crate) additional_roots: Vec<String>,
    /// Analysis-only fallback directories (see [`load_lookup_script_roots`]),
    /// searched only after those, never linted, and ignored by
    /// `conflicting_script_versions`.
    pub(crate) lookup_roots: Vec<String>,
    /// See [`load_compiler_path`].
    pub(crate) compiler_path: String,
    /// See [`load_compile_check`]. Together with [`Self::compiler_path`],
    /// controls whether PapyrusCompiler.exe's own errors are merged in too
    /// — see [`lint_with_compile_check`].
    pub(crate) compile_check: bool,
}

impl ProjectLintContext {
    pub(crate) fn function_table(&self) -> Arc<RwLock<function_table::FunctionTable>> {
        project_function_table_for_game(
            self.config.game,
            self.root.clone(),
            self.additional_roots.clone(),
            self.lookup_roots.clone(),
        )
    }
}

/// Compiles the `.psc` file at `path` using the compiler executable at
/// `compiler_path` (see [`load_compiler_path`]/[`resolve_compiler_path`]
/// for how the frontend obtains that path), selecting compiler flags for
/// `game`. `additional_roots` are the
/// project's configured additional script roots (see
/// [`load_script_roots`]), included in the compiler's `-i` argument
/// alongside the two conventional source directories. Returns an error if
/// `compiler_path` is blank/unconfigured or the compiler process itself
/// couldn't be run; a script that fails to compile is still reported as
/// `Ok`, with [`compiler::CompileOutcome::success`] false and the
/// compiler's stdout/stderr carrying the reported errors.
#[tauri::command(async)]
pub(crate) fn compile_psc_file(
    path: String,
    game: papyrus_lints::Game,
    compiler_path: String,
    additional_roots: Vec<String>,
) -> Result<compiler::CompileOutcome, String> {
    let compiler_path = compiler_path.trim();
    if compiler_path.is_empty() {
        return Err(
            "No PapyrusCompiler.exe path is configured. Set one in the Settings tab.".to_string(),
        );
    }

    compiler::compile_psc_file(
        game,
        Path::new(compiler_path),
        &PathBuf::from(path),
        &additional_roots,
    )
}

/// Computes `path`'s project diagnostics — if `rules.conflicting_script_versions`
/// is enabled, same-named byte-different scripts elsewhere among `function_table`'s
/// search roots (see [`script_locator::conflicting_script_versions`]); if
/// `rules.stale_compiled_output` is enabled, `path`'s conventionally located
/// compiled `.pex` being older than it (see [`stale_pex::check`]); if
/// `rules.script_filename_mismatch` is enabled, `path`'s file stem against
/// `source`'s declared `ScriptName` (see [`script_filename_mismatch::check`],
/// which reads that name from the lexer tokens)
/// — then runs every lint rule against `source` (via `function_table`, for
/// cross-script lookups) with those project diagnostics merged in via
/// [`papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`],
/// rather than appended to that call's own result afterward, so a
/// `@disable`/`@disable-file` directive naming one of them is honored and
/// counted as used by the `unused-disable` lint rather than incorrectly
/// flagged as unused. Then, if `compile_check` is set and `compiler_path`
/// isn't blank, also runs PapyrusCompiler.exe against the script at `path`
/// (into a throwaway temporary directory — see [`compiler::check_psc_file`])
/// and appends any errors it reports (see
/// [`compile_diagnostics::parse_compile_errors`]) to the result, so a
/// syntax mistake the compiler itself rejects but the lint engine's own,
/// more forgiving parser doesn't still shows up as a diagnostic. A
/// compiler that can't be run at all (a missing/misconfigured
/// `compiler_path`) is silently left out rather than failing the whole
/// lint — the engine's own diagnostics are still worth reporting either
/// way.
pub(crate) fn lint_with_compile_check<E: papyrus_lints::ExternalSignatures>(
    path: &Path,
    source: &str,
    context: &ProjectLintContext,
    function_table: &mut E,
) -> Vec<papyrus_lints::Diagnostic> {
    // Computed up front and merged in via
    // `lint_with_external_arguments_and_extra_diagnostics` below, rather than
    // appended to that call's own result afterward, so a `@disable`/
    // `@disable-file` directive naming one of these path-dependent
    // diagnostics is honored *and* counted as used by the `unused-disable`
    // lint instead of being incorrectly flagged as unused (see
    // `papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`'s
    // own docs).
    let mut project_diagnostics = Vec::new();
    if context.config.rules.conflicting_script_versions {
        let index =
            script_locator::build_script_index(Path::new(&context.root), &context.additional_roots);
        let files = script_locator::project_files(
            index.into_values().flatten(),
            Path::new(&context.root),
            false,
            context.config.game,
        );
        project_diagnostics.extend(papyrus_lints::conflicting_script_versions::check(
            path,
            &papyrus_lint_core::content_hash::md5_hex(source),
            &files,
        ));
    }
    if context.config.rules.stale_compiled_output {
        project_diagnostics.extend(stale_pex::check(path));
    }
    if context.config.rules.script_filename_mismatch {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            if let Ok(tokens) = papyrus_parser::tokenize(source) {
                project_diagnostics.extend(script_filename_mismatch::check(stem, &tokens));
            }
        }
    }
    let mut diagnostics = papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
        source,
        &context.config,
        function_table,
        project_diagnostics,
    );

    let compiler_path = context.compiler_path.trim();
    if context.compile_check && !compiler_path.is_empty() {
        if let Ok(outcome) = compiler::check_psc_file(
            context.config.game,
            Path::new(compiler_path),
            path,
            &context.additional_roots,
        ) {
            if !outcome.success {
                diagnostics.extend(compile_diagnostics::parse_compile_errors(&outcome));
            }
        }
    }

    diagnostics
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring the semicolon style `context.config` selects. See
/// [`ProjectLintContext`] for `root`/`additional_roots`/`lookup_roots`/
/// `compiler_path`/`compile_check`.
#[tauri::command(async)]
pub(crate) fn lint_psc_file(
    path: String,
    context: ProjectLintContext,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    ast_cache::ensure_primed_for_game(context.config.game, path, &source);
    let function_table = context.function_table();
    let mut shared = function_table::SharedFunctionTable(function_table.as_ref());
    Ok(lint_with_compile_check(
        path,
        &source,
        &context,
        &mut shared,
    ))
}

/// Parses every one of `paths` up front and closes over the type names in
/// those ASTs (mirroring `PapyrusLinterCLI`'s parse phase — see
/// [`function_table::FunctionTable::parse_type_closure`]), then preloads
/// `context`'s shared function table from the result. Referenced scripts
/// that are not in `paths` — project files, lookup-root files, and bundled
/// vanilla/extender scripts — are cached for analysis only and are not
/// linted. Resolving one of them from a later per-file [`lint_psc_file`]
/// is a cache hit rather than a write-locked on-demand parse. A path that
/// fails to read or parse is simply left out of the seed preload (the
/// frontend's own per-file call still reports that error); a referenced
/// name that cannot be resolved is cached unresolved so lint does not
/// retry it. This command never fails outright.
#[tauri::command(async)]
pub(crate) fn preload_project_scripts(paths: Vec<String>, context: ProjectLintContext) {
    let function_table = context.function_table();
    let script_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let game = context.config.game;

    let closed = {
        let table = function_table
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        table.parse_type_closure(
            &script_paths,
            function_table::TypeClosureOptions {
                threads: papyrus_lint_core::parallel::default_thread_count(),
                total_files: None,
            },
            |path| parse_project_script(game, path),
            |parsed| parsed.ast.as_ref(),
            || {},
        )
    };

    let entries: Vec<function_table::PreloadedScript> = script_paths
        .iter()
        .zip(closed.seeds.iter())
        .filter_map(|(path, parsed)| {
            let source = parsed.source.as_deref()?;
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(function_table::PreloadedScript {
                path,
                name_lower,
                ast: parsed.ast.as_ref(),
                source,
            })
        })
        .collect();

    let mut table = function_table
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.preload(entries);
    table.preload_dependencies(&closed.dependencies);
    table.preload_name_slots(&closed.bundled, &closed.unresolved);
}

struct ProjectScriptParse {
    source: Option<String>,
    ast: Option<papyrus_parser::ast::Script>,
}

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompletionQuery {
    receiver_type: String,
    prefix: String,
    prefix_start: usize,
}

fn blank_comments(source: &str) -> String {
    let block = regex::Regex::new(r"(?s);/.*?(?:/;|\z)").expect("valid block comment regex");
    let brace = regex::Regex::new(r"(?s)\{.*?(?:\}|\z)").expect("valid brace comment regex");
    let line = regex::Regex::new(r";[^\n]*").expect("valid line comment regex");
    let blank = |captures: &regex::Captures<'_>| {
        captures[0]
            .chars()
            .map(|character| if character == '\n' { '\n' } else { ' ' })
            .collect::<String>()
    };
    let source = block.replace_all(source, &blank);
    let source = brace.replace_all(&source, &blank);
    line.replace_all(&source, blank).into_owned()
}

fn declared_type(source: &str, receiver: &str) -> Option<String> {
    let identifier = r"[A-Za-z_]\w*";
    let clean = blank_comments(source);
    let receiver = receiver.to_ascii_lowercase();

    let script = regex::Regex::new(&format!(
        r"(?im)^\s*ScriptName\s+({identifier})(?:\s+Extends\s+({identifier}))?"
    ))
    .expect("valid script declaration regex");
    if let Some(captures) = script.captures(&clean) {
        if receiver == "self" {
            return Some(captures[1].to_string());
        }
        if receiver == "parent" {
            return captures.get(2).map(|parent| parent.as_str().to_string());
        }
    }

    let declarations = regex::Regex::new(&format!(
        r"(?im)^\s*({identifier})(?:\[\])?\s+(?:Property\s+)?({identifier})\s*(?:=|;|\bAuto\b|$)"
    ))
    .expect("valid variable declaration regex");
    let is_keyword = |word: &str| {
        matches!(
            word.to_ascii_lowercase().as_str(),
            "scriptname"
                | "extends"
                | "hidden"
                | "conditional"
                | "import"
                | "function"
                | "endfunction"
                | "event"
                | "endevent"
                | "property"
                | "endproperty"
                | "auto"
                | "autoreadonly"
                | "global"
                | "native"
                | "return"
                | "if"
                | "elseif"
                | "else"
                | "endif"
                | "while"
                | "endwhile"
                | "state"
                | "endstate"
                | "new"
                | "as"
                | "true"
                | "false"
                | "none"
                | "self"
                | "parent"
                | "length"
                | "debugonly"
                | "betaonly"
        )
    };
    for captures in declarations.captures_iter(&clean) {
        if captures[2].eq_ignore_ascii_case(&receiver)
            && !is_keyword(&captures[1])
            && !is_keyword(&captures[2])
        {
            return Some(captures[1].to_string());
        }
    }

    let headers = regex::Regex::new(&format!(
        r"(?i)\b(?:Function|Event)\s+{identifier}\s*\(([^)]*)\)"
    ))
    .expect("valid function header regex");
    let parameter = regex::Regex::new(&format!(r"^\s*({identifier})(?:\[\])?\s+({identifier})"))
        .expect("valid parameter regex");
    for header in headers.captures_iter(&clean) {
        for value in header[1].split(',') {
            if let Some(captures) = parameter.captures(value) {
                if captures[2].eq_ignore_ascii_case(&receiver)
                    && !is_keyword(&captures[1])
                    && !is_keyword(&captures[2])
                {
                    return Some(captures[1].to_string());
                }
            }
        }
    }
    None
}

/// Resolves the type and member-name prefix immediately before `cursor_index`.
/// This deliberately scans incomplete source instead of requiring a valid AST,
/// because a trailing member-access dot is not valid Papyrus yet.
#[tauri::command]
pub(crate) fn resolve_completion_query(
    source: String,
    cursor_index: usize,
) -> Option<CompletionQuery> {
    // Browser selection offsets count UTF-16 code units; Rust string slices
    // use UTF-8 byte offsets. Translate without letting non-ASCII comments or
    // string literals before the cursor suppress otherwise valid completion.
    let mut utf16_offset = 0;
    let mut byte_offset = None;
    for (index, character) in source.char_indices() {
        if utf16_offset == cursor_index {
            byte_offset = Some(index);
            break;
        }
        utf16_offset += character.len_utf16();
    }
    if utf16_offset == cursor_index {
        byte_offset.get_or_insert(source.len());
    }
    let before = source.get(..byte_offset?)?;
    let receiver = regex::Regex::new(r"([A-Za-z_]\w*)(?:\s*\[[^\[\]]*\])?\s*\.(\w*)$")
        .expect("valid completion receiver regex");
    let captures = receiver.captures(before)?;
    let prefix = captures[2].to_string();
    let receiver_type = declared_type(&source, &captures[1])?;
    Some(CompletionQuery {
        receiver_type,
        prefix_start: cursor_index - prefix.len(),
        prefix,
    })
}

fn parse_project_script(game: papyrus_lints::Game, path: &Path) -> ProjectScriptParse {
    let source = read_psc_source(path).ok();
    let ast = source.as_ref().and_then(|source| {
        if let Some(cached) = ast_cache::get_for_game(game, path, source) {
            return Some(cached);
        }
        let parsed = papyrus_parser::parse(source).ok()?;
        ast_cache::put_for_game(game, path, source, &parsed);
        if let Ok(tokens) = papyrus_parser::tokenize(source) {
            ast_cache::put_tokens_for_game(game, path, source, &tokens);
        }
        Some(parsed)
    });
    ProjectScriptParse { source, ast }
}

/// Lists every function and property available on an object of type
/// `type_name` (including those inherited via `Extends`), for driving the
/// code viewer's editor autocompletion. See [`lint_psc_file`] for
/// `root`/`additional_roots`.
#[tauri::command(async)]
pub(crate) fn list_script_members(
    root: String,
    type_name: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Vec<function_table::Member> {
    let function_table = project_function_table(root, additional_roots, lookup_roots);
    function_table::SharedFunctionTable(function_table.as_ref()).list_members(&type_name)
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;

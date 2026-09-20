//! Project-aware linting, compile-check, and compile commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{
    ast_cache, compile_diagnostics, compiler, function_table, script_filename_mismatch,
    script_locator, stale_pex,
};
use serde::{Deserialize, Serialize};

/// Identity of one desktop-app [`function_table::FunctionTable`]: project
/// root plus the two configured search-root lists. Concurrent Tauri
/// commands for the same project reuse one `RwLock`-guarded table through
/// [`function_table::SharedFunctionTable`], matching the CLI's `--threads`
/// workers rather than building an empty table per file. The exclusive lock
/// is taken only when a lookup still has to parse a script.
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
/// for how the frontend obtains that path). `additional_roots` are the
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
/// `rules.script_filename_mismatch` is enabled, `path`'s file name against
/// `source`'s declared `ScriptName` (see [`script_filename_mismatch::check`])
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
        project_diagnostics.extend(script_locator::conflicting_script_versions(
            path,
            Path::new(&context.root),
            &context.additional_roots,
        ));
    }
    if context.config.rules.stale_compiled_output {
        project_diagnostics.extend(stale_pex::check(path));
    }
    if context.config.rules.script_filename_mismatch {
        project_diagnostics.extend(script_filename_mismatch::check(path, source));
    }
    let mut diagnostics = papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
        source,
        &context.config,
        function_table,
        project_diagnostics,
    );

    let compiler_path = context.compiler_path.trim();
    if context.compile_check && !compiler_path.is_empty() {
        if let Ok(outcome) =
            compiler::check_psc_file(Path::new(compiler_path), path, &context.additional_roots)
        {
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
    ast_cache::ensure_primed_for_game(context.config.game.as_str(), path, &source);
    let function_table = context.function_table();
    let mut shared = function_table::SharedFunctionTable(function_table.as_ref());
    Ok(lint_with_compile_check(
        path,
        &source,
        &context,
        &mut shared,
    ))
}

/// Parses every one of `paths` up front (in parallel, mirroring
/// `PapyrusLinterCLI`'s own parse phase — see
/// `papyrus_lint_cli::run_lint_command`'s module docs) and preloads
/// `context`'s shared function table ([`function_table::FunctionTable::preload`])
/// from the result, before the frontend's own per-file `parse_psc_file`/
/// [`lint_psc_file`] pair runs across the same batch (see `parsePscFiles` in
/// `app/src/drop.ts`). Resolving an `Extends`/type reference to another
/// script in `paths` is then a cache hit from the start for every one of
/// those per-file calls, rather than a write-locked, on-demand parse the
/// first one to need it triggers. A path that fails to read or parse is
/// simply left out of the preload -- the frontend's own per-file call still
/// reports that error as usual -- so this command never fails outright.
#[tauri::command(async)]
pub(crate) fn preload_project_scripts(paths: Vec<String>, context: ProjectLintContext) {
    let function_table = context.function_table();
    let script_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

    // Mirrors `parse_psc_file`'s own get-or-parse-and-cache logic (see
    // `files.rs`), run for every script at once instead of one Tauri
    // command per file, so this reuses whatever the disk-backed
    // `ast_cache` already knows and only pays for a fresh parse on an
    // actual cache miss.
    let parsed: Vec<(PathBuf, Option<String>, Option<papyrus_parser::ast::Script>)> =
        papyrus_lint_core::parallel::map_in_parallel(
            script_paths,
            papyrus_lint_core::parallel::default_thread_count(),
            |path| {
                let source = read_psc_source(&path).ok();
                let ast = source.as_ref().and_then(|source| {
                    if let Some(cached) =
                        ast_cache::get_for_game(context.config.game.as_str(), &path, source)
                    {
                        return Some(cached);
                    }
                    let parsed = papyrus_parser::parse(source).ok()?;
                    ast_cache::put_for_game(context.config.game.as_str(), &path, source, &parsed);
                    if let Ok(tokens) = papyrus_parser::tokenize(source) {
                        ast_cache::put_tokens_for_game(
                            context.config.game.as_str(),
                            &path,
                            source,
                            &tokens,
                        );
                    }
                    Some(parsed)
                });
                (path, source, ast)
            },
        );

    let entries: Vec<function_table::PreloadedScript> = parsed
        .iter()
        .filter_map(|(path, source, ast)| {
            let source = source.as_deref()?;
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(function_table::PreloadedScript {
                path,
                name_lower,
                ast: ast.as_ref(),
                source,
            })
        })
        .collect();

    let mut table = function_table
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.preload(entries);
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

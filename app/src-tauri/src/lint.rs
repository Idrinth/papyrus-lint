//! Project-aware linting, compile-check, and compile commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{
    ast_cache, compile_diagnostics, compiler, function_table, script_filename_mismatch,
    script_locator, stale_pex,
};

/// Identity of one desktop-app [`function_table::FunctionTable`]: project
/// root plus the two configured search-root lists. Concurrent Tauri
/// commands for the same project reuse one `Mutex`-guarded table through
/// [`function_table::SharedFunctionTable`], matching the CLI's `--threads`
/// workers rather than building an empty table per file.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SharedTableKey {
    root: PathBuf,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
}

fn shared_tables(
) -> &'static Mutex<HashMap<SharedTableKey, Arc<Mutex<function_table::FunctionTable>>>> {
    static TABLES: OnceLock<
        Mutex<HashMap<SharedTableKey, Arc<Mutex<function_table::FunctionTable>>>>,
    > = OnceLock::new();
    TABLES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn project_function_table(
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Arc<Mutex<function_table::FunctionTable>> {
    let key = SharedTableKey {
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
            Arc::new(Mutex::new(
                function_table::FunctionTable::new_with_additional_roots(
                    PathBuf::from(root),
                    additional_roots,
                )
                .with_lookup_roots(lookup_roots),
            ))
        })
        .clone()
}

fn lock_function_table(
    table: &Mutex<function_table::FunctionTable>,
) -> std::sync::MutexGuard<'_, function_table::FunctionTable> {
    table
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    config: &papyrus_lints::Config,
    function_table: &mut E,
    root: &Path,
    additional_roots: &[String],
    compiler_path: &str,
    compile_check: bool,
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
    if config.rules.conflicting_script_versions {
        project_diagnostics.extend(script_locator::conflicting_script_versions(
            path,
            root,
            additional_roots,
        ));
    }
    if config.rules.stale_compiled_output {
        project_diagnostics.extend(stale_pex::check(path));
    }
    if config.rules.script_filename_mismatch {
        project_diagnostics.extend(script_filename_mismatch::check(path, source));
    }
    let mut diagnostics = papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
        source,
        config,
        function_table,
        project_diagnostics,
    );

    let compiler_path = compiler_path.trim();
    if compile_check && !compiler_path.is_empty() {
        if let Ok(outcome) =
            compiler::check_psc_file(Path::new(compiler_path), path, additional_roots)
        {
            if !outcome.success {
                diagnostics.extend(compile_diagnostics::parse_compile_errors(&outcome));
            }
        }
    }

    diagnostics
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring the semicolon style `config` selects. `root` is the project
/// root (conventionally the directory containing the `.achlist` file); it
/// lets the "Argument type check" lint resolve calls to functions declared
/// on other scripts under `root`, and lets the "Return type check" lint
/// accept a returned value whose script under `root` extends the declared
/// return type. `additional_roots` are the project's configured additional
/// script roots (see [`load_script_roots`]), searched the same way
/// alongside `root`'s conventional source directories. `lookup_roots` are
/// analysis-only fallback directories (see [`load_lookup_script_roots`]),
/// searched only after those, never linted, and ignored by
/// `conflicting_script_versions`. `compiler_path` and
/// `compile_check` (see [`load_compiler_path`]/[`load_compile_check`])
/// control whether PapyrusCompiler.exe's own errors are merged in too —
/// see [`lint_with_compile_check`].
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn lint_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    ast_cache::ensure_primed(path, &source);
    let function_table =
        project_function_table(root.clone(), additional_roots.clone(), lookup_roots);
    let mut shared = function_table::SharedFunctionTable(function_table.as_ref());
    Ok(lint_with_compile_check(
        path,
        &source,
        &config,
        &mut shared,
        Path::new(&root),
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
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
    lock_function_table(function_table.as_ref()).list_members(&type_name)
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;

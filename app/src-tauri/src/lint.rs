//! Project-aware linting, compile-check, and compile commands.

use std::path::{Path, PathBuf};

use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{
    ast_cache, compile_diagnostics, compiler, function_table, script_filename_mismatch,
    script_locator, stale_pex,
};

pub(crate) fn project_function_table(
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> function_table::FunctionTable {
    function_table::FunctionTable::new_with_additional_roots(PathBuf::from(root), additional_roots)
        .with_lookup_roots(lookup_roots)
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
pub(crate) fn lint_with_compile_check(
    path: &Path,
    source: &str,
    config: &papyrus_lints::Config,
    function_table: &mut function_table::FunctionTable,
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
            function_table.root(),
            function_table.additional_roots(),
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
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    Ok(lint_with_compile_check(
        path,
        &source,
        &config,
        &mut function_table,
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
    let mut function_table = project_function_table(root, additional_roots, lookup_roots);
    function_table.list_members(&type_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn compile_psc_file_rejects_a_blank_compiler_path_before_spawning() {
        assert!(
            compile_psc_file("Example.psc".to_string(), "  \t".to_string(), Vec::new()).is_err()
        );
    }

    #[test]
    fn compile_psc_file_reports_a_spawn_error_from_the_compiler_module() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let script_path = source_dir.join("Example.psc");
        std::fs::write(&script_path, "ScriptName Example\n").unwrap();

        let error = compile_psc_file(
            script_path.to_string_lossy().into_owned(),
            dir.path()
                .join("missing-compiler")
                .to_string_lossy()
                .into_owned(),
            Vec::new(),
        )
        .unwrap_err();

        assert!(error.contains("failed to run"));
    }

    #[test]
    fn lint_psc_file_lints_source_from_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(
            &path,
            "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
        )
        .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == papyrus_lints::forbidden_functions::RULE));
    }

    #[test]
    fn lint_psc_file_ignores_compile_check_when_disabled() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            "/does/not/matter".to_string(),
            false,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn lint_psc_file_ignores_compile_check_when_no_compiler_path_is_set() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            "   ".to_string(),
            true,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn lint_psc_file_flags_a_script_newer_than_its_compiled_pex() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        let pex_path = dir.path().join("Scripts/Example.pex");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        std::fs::write(&pex_path, "").unwrap();

        let now = std::time::SystemTime::now();
        std::fs::File::open(&pex_path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(60))
            .unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(now)
            .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == stale_pex::RULE
                && diagnostic.message.starts_with("[info]")));
    }

    #[test]
    fn lint_psc_file_ignores_stale_compiled_output_when_disabled() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        let pex_path = dir.path().join("Scripts/Example.pex");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        std::fs::write(&pex_path, "").unwrap();

        let now = std::time::SystemTime::now();
        std::fs::File::open(&pex_path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(60))
            .unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(now)
            .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                stale_compiled_output: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != stale_pex::RULE));
    }

    #[test]
    fn lint_psc_file_reports_script_filename_mismatch() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule
            == script_filename_mismatch::RULE
            && diagnostic.message.starts_with("[error]")));
    }

    #[test]
    fn lint_psc_file_ignores_script_filename_mismatch_when_disabled() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                script_filename_mismatch: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
    }

    #[test]
    fn lint_psc_file_honors_a_disable_comment_for_script_filename_mismatch() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(
            &path,
            "ScriptName Example ; @disable script-filename-mismatch\n",
        )
        .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
    }

    #[test]
    fn lint_psc_file_honors_a_disable_file_comment_for_script_filename_mismatch() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(
            &path,
            "ScriptName Example\n; @disable-file script-filename-mismatch\n",
        )
        .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
    }

    #[test]
    fn lint_psc_file_reports_conflicting_script_versions() {
        let dir = tempdir().unwrap();
        let first_root = dir.path().join("scripts/source");
        let second_root = dir.path().join("source/scripts");
        std::fs::create_dir_all(&first_root).unwrap();
        std::fs::create_dir_all(&second_root).unwrap();
        let path = first_root.join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        std::fs::write(
            second_root.join("Example.psc"),
            "ScriptName Example\n; a different version\n",
        )
        .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE
        }));
    }

    #[test]
    fn lint_psc_file_ignores_conflicting_script_versions_when_disabled() {
        let dir = tempdir().unwrap();
        let first_root = dir.path().join("scripts/source");
        let second_root = dir.path().join("source/scripts");
        std::fs::create_dir_all(&first_root).unwrap();
        std::fs::create_dir_all(&second_root).unwrap();
        let path = first_root.join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        std::fs::write(
            second_root.join("Example.psc"),
            "ScriptName Example\n; a different version\n",
        )
        .unwrap();
        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                conflicting_script_versions: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.rule != script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE
        }));
    }

    #[test]
    fn lint_psc_file_stale_compiled_output_disable_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        let pex_path = dir.path().join("Scripts/Example.pex");
        std::fs::write(
            &path,
            "ScriptName Example ; @disable stale-compiled-output\n",
        )
        .unwrap();
        std::fs::write(&pex_path, "").unwrap();

        let now = std::time::SystemTime::now();
        std::fs::File::open(&pex_path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(60))
            .unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(now)
            .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != stale_pex::RULE));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    fn lint_psc_file_stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        let pex_path = dir.path().join("Scripts/Example.pex");
        std::fs::write(
            &path,
            "ScriptName Example\n; @disable-file stale-compiled-output\n",
        )
        .unwrap();
        std::fs::write(&pex_path, "").unwrap();

        let now = std::time::SystemTime::now();
        std::fs::File::open(&pex_path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(60))
            .unwrap();
        std::fs::File::open(&path)
            .unwrap()
            .set_modified(now)
            .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != stale_pex::RULE));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    fn lint_psc_file_conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let first_root = dir.path().join("scripts/source");
        let second_root = dir.path().join("source/scripts");
        std::fs::create_dir_all(&first_root).unwrap();
        std::fs::create_dir_all(&second_root).unwrap();
        let path = first_root.join("Example.psc");
        std::fs::write(
            &path,
            "ScriptName Example ; @disable conflicting-script-versions\n",
        )
        .unwrap();
        std::fs::write(
            second_root.join("Example.psc"),
            "ScriptName Example\n; a different version\n",
        )
        .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.rule != script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE
        }));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    fn lint_psc_file_conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let first_root = dir.path().join("scripts/source");
        let second_root = dir.path().join("source/scripts");
        std::fs::create_dir_all(&first_root).unwrap();
        std::fs::create_dir_all(&second_root).unwrap();
        let path = first_root.join("Example.psc");
        std::fs::write(
            &path,
            "ScriptName Example\n; @disable-file conflicting-script-versions\n",
        )
        .unwrap();
        std::fs::write(
            second_root.join("Example.psc"),
            "ScriptName Example\n; a different version\n",
        )
        .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.rule != script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE
        }));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    fn lint_psc_file_script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(
            &path,
            "ScriptName Example ; @disable script-filename-mismatch\n",
        )
        .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    fn lint_psc_file_script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Other.psc");
        std::fs::write(
            &path,
            "ScriptName Example\n; @disable-file script-filename-mismatch\n",
        )
        .unwrap();

        let config = papyrus_lints::Config {
            rules: papyrus_lints::config::Rules {
                unused_disable: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_disable::RULE));
    }

    #[test]
    #[cfg(unix)]
    fn lint_psc_file_merges_in_compiler_reported_errors_when_enabled() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        let compiler_path = dir.path().join("compiler.sh");
        std::fs::write(
            &compiler_path,
            "#!/bin/sh\necho \"Example.psc(3,4): no viable alternative at character ';'\" >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            compiler_path.to_string_lossy().into_owned(),
            true,
        )
        .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == compile_diagnostics::RULE
                && diagnostic.line == 3
                && diagnostic.column == 4
        }));
    }

    #[test]
    #[cfg(unix)]
    fn lint_psc_file_omits_compiler_diagnostics_when_the_compiler_reports_success() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        let compiler_path = dir.path().join("compiler.sh");
        std::fs::write(&compiler_path, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            compiler_path.to_string_lossy().into_owned(),
            true,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .all(|d| d.rule != compile_diagnostics::RULE));
    }

    #[test]
    fn lint_psc_file_does_not_fail_when_the_configured_compiler_cannot_be_run() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            dir.path()
                .join("missing-compiler")
                .to_string_lossy()
                .into_owned(),
            true,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn list_script_members_reports_functions_and_properties_including_inherited_ones() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("scripts/source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("Base.psc"),
            "ScriptName Base\n\nBool Property IsAwesome Auto\n",
        )
        .unwrap();
        std::fs::write(
            source_dir.join("Child.psc"),
            "ScriptName Child Extends Base\n\nInt Function DoThing(Float a)\nEndFunction\n",
        )
        .unwrap();

        let members = list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Child".to_string(),
            Vec::new(),
            Vec::new(),
        );

        let names: std::collections::HashSet<_> =
            members.iter().map(function_table::Member::name).collect();
        assert_eq!(
            names,
            std::collections::HashSet::from(["DoThing", "IsAwesome"])
        );
    }

    #[test]
    fn list_script_members_resolves_a_type_via_an_additional_script_root() {
        let dir = tempdir().unwrap();
        let shared = tempdir().unwrap();
        std::fs::write(
            shared.path().join("Shared.psc"),
            "ScriptName Shared\n\nInt Property MyValue Auto\n",
        )
        .unwrap();

        let members = list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Shared".to_string(),
            vec![shared.path().to_string_lossy().into_owned()],
            Vec::new(),
        );

        assert_eq!(members.len(), 1);
    }

    #[test]
    fn list_script_members_resolves_a_type_via_a_lookup_script_root() {
        let dir = tempdir().unwrap();
        let vanilla = tempdir().unwrap();
        std::fs::write(
            vanilla.path().join("Shared.psc"),
            "ScriptName Shared\n\nInt Property MyValue Auto\n",
        )
        .unwrap();

        let members = list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Shared".to_string(),
            Vec::new(),
            vec![vanilla.path().to_string_lossy().into_owned()],
        );

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].name(), "MyValue");
    }

    #[test]
    fn list_script_members_is_empty_for_an_unresolvable_type() {
        let dir = tempdir().unwrap();

        assert!(list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Missing".to_string(),
            Vec::new(),
            Vec::new(),
        )
        .is_empty());
    }

    #[test]
    fn list_script_members_matches_type_names_case_insensitively() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("scripts/source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("Example.psc"),
            "ScriptName Example\n\nString Property DisplayName Auto\n",
        )
        .unwrap();

        let members = list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "eXaMpLe".to_string(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].name(), "DisplayName");
    }

    #[test]
    #[cfg(unix)]
    fn compile_command_trims_the_executable_path_and_returns_its_output() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let script_path = source_dir.join("Example.psc");
        std::fs::write(&script_path, "").unwrap();
        let compiler_path = dir.path().join("compiler.sh");
        std::fs::write(&compiler_path, "#!/bin/sh\necho command wrapper\n").unwrap();
        std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let outcome = compile_psc_file(
            script_path.to_string_lossy().into_owned(),
            format!("  {}  ", compiler_path.display()),
            Vec::new(),
        )
        .unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.stdout, "command wrapper\n");
    }

    #[test]
    #[cfg(unix)]
    fn compile_command_returns_a_failed_compiler_outcome() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let script_path = source_dir.join("Example.psc");
        std::fs::write(&script_path, "ScriptName Example\n").unwrap();
        let compiler_path = dir.path().join("compiler.sh");
        std::fs::write(
            &compiler_path,
            "#!/bin/sh\necho compile failed >&2\nexit 1\n",
        )
        .unwrap();
        std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let outcome = compile_psc_file(
            script_path.to_string_lossy().into_owned(),
            compiler_path.to_string_lossy().into_owned(),
            Vec::new(),
        )
        .unwrap();

        assert!(!outcome.success);
        assert_eq!(outcome.stderr, "compile failed\n");
    }

    #[test]
    #[cfg(unix)]
    fn compile_command_forwards_additional_script_roots() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("Scripts/Source");
        let additional_root = dir.path().join("Shared Scripts");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&additional_root).unwrap();
        let script_path = source_dir.join("Example.psc");
        std::fs::write(&script_path, "ScriptName Example\n").unwrap();
        let compiler_path = dir.path().join("compiler.sh");
        std::fs::write(&compiler_path, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let outcome = compile_psc_file(
            script_path.to_string_lossy().into_owned(),
            compiler_path.to_string_lossy().into_owned(),
            vec![additional_root.to_string_lossy().into_owned()],
        )
        .unwrap();

        assert!(outcome.success);
        assert!(outcome
            .stdout
            .contains(&additional_root.to_string_lossy().into_owned()));
        assert!(outcome.stdout.contains("Example.psc"));
    }

    #[test]
    fn lint_psc_file_resolves_argument_types_through_additional_script_roots() {
        let extra = tempdir().unwrap();
        std::fs::write(
            extra.path().join("Helpers.psc"),
            "ScriptName Helpers\n\nFunction NeedInt(Int count)\nEndFunction\n",
        )
        .unwrap();
        let dir = tempdir().unwrap();
        let path = dir.path().join("Caller.psc");
        std::fs::write(
            &path,
            "ScriptName Caller\n\nFunction Run(Helpers helper)\n    helper.NeedInt(\"nope\")\nEndFunction\n",
        )
        .unwrap();
        let extra_root = extra.path().to_string_lossy().into_owned();

        let without_roots = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();
        assert!(without_roots
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::argument_types::RULE));

        let with_roots = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            vec![extra_root],
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();
        assert!(with_roots.iter().any(|diagnostic| {
            diagnostic.rule == papyrus_lints::argument_types::RULE
                && diagnostic.message.contains("expects Int")
                && diagnostic.message.contains("got String")
        }));
    }

    #[test]
    fn lint_psc_file_reports_conflicting_script_versions_in_an_additional_root() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("scripts/source");
        std::fs::create_dir_all(&source_dir).unwrap();
        let path = source_dir.join("Example.psc");
        std::fs::write(&path, "ScriptName Example\n").unwrap();
        let extra = tempdir().unwrap();
        std::fs::write(
            extra.path().join("Example.psc"),
            "ScriptName Example\n; a different version\n",
        )
        .unwrap();

        let diagnostics = lint_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            vec![extra.path().to_string_lossy().into_owned()],
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE
        }));
    }
}

pub mod compile_diagnostics;
pub mod compiler;
pub mod pex_header;

use std::path::{Path, PathBuf};

use papyrus_lint_core::source_encoding::{
    read_psc_source, read_psc_source_with_encoding, write_psc_source,
};
use papyrus_lint_core::{achlist, ast_cache, config, function_table, script_locator};

#[derive(Debug, PartialEq, serde::Serialize)]
struct ProjectInfo {
    detected_script_roots: Vec<String>,
    used_configuration_file: Option<String>,
}

/// Returns the desktop app's version (from `app/src-tauri/Cargo.toml`, kept in
/// sync with `package.json`/`tauri.conf.json` at release time), so the
/// frontend can display it to the user.
#[tauri::command]
fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Parses the `.achlist` file at `path` and returns the resolved paths it lists.
#[tauri::command]
fn parse_achlist_file(path: String) -> Result<Vec<String>, String> {
    let entries = achlist::parse_achlist(&PathBuf::from(path)).map_err(|err| err.to_string())?;

    Ok(entries
        .into_iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
fn parse_papyrus_script(source: &str) -> Result<papyrus_parser::ast::Script, String> {
    papyrus_parser::parse(source).map_err(|e| e.to_string())
}

#[tauri::command]
fn lint_papyrus_script(
    source: &str,
    config: papyrus_lints::Config,
) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lints::lint(source, &config)
}

/// Reads the `.psc` file at `path` and parses it into a `Script` AST,
/// reusing a disk-backed cache (see [`ast_cache`]) keyed by `path`'s
/// content and modification time when the file hasn't changed since it was
/// last parsed.
#[tauri::command]
fn parse_psc_file(path: String) -> Result<papyrus_parser::ast::Script, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;

    if let Some(cached) = ast_cache::get(path, &source) {
        return Ok(cached);
    }

    let script = papyrus_parser::parse(&source).map_err(|err| err.to_string())?;
    ast_cache::put(path, &source, &script);
    Ok(script)
}

/// Reads the `.psc` file at `path` and returns its raw source text, for the
/// frontend's syntax-highlighted code viewer.
#[tauri::command]
fn read_psc_file(path: String) -> Result<String, String> {
    read_psc_source(Path::new(&path)).map_err(|err| err.to_string())
}

/// Writes `contents` to the `.psc` file at `path`, replacing it on disk.
/// Used by the frontend's code viewer to persist edits made in its edit
/// mode.
#[tauri::command]
fn write_psc_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|err| err.to_string())
}

/// Looks for a papyrus-lint YAML config file in `dir` (conventionally the
/// directory containing the `.achlist` file) and returns the lint
/// configuration it describes, falling back to the default configuration
/// if `dir` has no config file.
#[tauri::command]
fn load_lint_config(dir: String) -> Result<papyrus_lints::Config, String> {
    config::load_config(&PathBuf::from(dir))
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file (creating it,
/// as `papyrus-lint.yaml`, if `dir` has none yet), so the formatting
/// selected in the UI is remembered for next time.
#[tauri::command]
fn save_lint_config(dir: String, config: papyrus_lints::Config) -> Result<(), String> {
    config::save_config(&PathBuf::from(dir), &config)
}

/// Returns the PapyrusCompiler.exe path to use for `dir`'s project: an
/// explicit override saved to its papyrus-lint config file, or, absent
/// one, a path auto-detected at `../Papyrus Compiler/PapyrusCompiler.exe`
/// relative to `dir` (the directory containing the `.achlist` file).
/// Returns `null` if neither is available.
#[tauri::command]
fn load_compiler_path(dir: String) -> Result<Option<String>, String> {
    config::resolve_compiler_path(&PathBuf::from(dir))
}

/// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
/// papyrus-lint config file. Passing an empty (or blank) string clears
/// the override, reverting to auto-detection.
#[tauri::command]
fn save_compiler_path(dir: String, path: String) -> Result<(), String> {
    let path = path.trim();
    config::save_compiler_path(
        &PathBuf::from(dir),
        if path.is_empty() { None } else { Some(path) },
    )
}

/// Returns whether `dir`'s project enables running PapyrusCompiler.exe as
/// part of linting a dropped `.psc` (see [`lint_psc_file`]/
/// [`compiler::check_psc_file`]), `false` by default.
#[tauri::command]
fn load_compile_check(dir: String) -> Result<bool, String> {
    config::load_compile_check(&PathBuf::from(dir))
}

/// Persists whether `dir`'s project runs PapyrusCompiler.exe as part of
/// linting a dropped `.psc`.
#[tauri::command]
fn save_compile_check(dir: String, enabled: bool) -> Result<(), String> {
    config::save_compile_check(&PathBuf::from(dir), enabled)
}

/// Returns `dir`'s configured additional script root directories (see
/// [`papyrus_lint_core::script_locator`]), if any. These are searched
/// alongside the conventional `scripts/source`/`source/scripts` directories
/// when resolving cross-script lookups (the "Argument type check"/"Return
/// type check" lints, autocompletion) and are appended to the compiler's
/// `-i` argument.
#[tauri::command]
fn load_script_roots(dir: String) -> Result<Vec<String>, String> {
    config::load_script_roots(&PathBuf::from(dir))
}

/// Reports the project paths discovered by the backend for display in the
/// Settings tab. Only script search directories that exist are included.
#[tauri::command]
fn load_project_info(dir: String) -> Result<ProjectInfo, String> {
    let root = PathBuf::from(dir);
    let additional_roots = config::load_script_roots(&root)?;
    Ok(ProjectInfo {
        detected_script_roots: script_locator::detected_script_roots(&root, &additional_roots)
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        used_configuration_file: config::config_file_path(&root)
            .map(|path| path.to_string_lossy().into_owned()),
    })
}

/// Persists `roots` as `dir`'s configured additional script root
/// directories.
#[tauri::command]
fn save_script_roots(dir: String, roots: Vec<String>) -> Result<(), String> {
    config::save_script_roots(&PathBuf::from(dir), &roots)
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
#[tauri::command]
fn compile_psc_file(
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

/// Runs every lint rule against `source` (via `function_table`, for
/// cross-script lookups), then, if `compile_check` is set and
/// `compiler_path` isn't blank, also runs PapyrusCompiler.exe against the
/// script at `path` (into a throwaway temporary directory — see
/// [`compiler::check_psc_file`]) and appends any errors it reports (see
/// [`compile_diagnostics::parse_compile_errors`]) to the result, so a
/// syntax mistake the compiler itself rejects but the lint engine's own,
/// more forgiving parser doesn't still shows up as a diagnostic. A
/// compiler that can't be run at all (a missing/misconfigured
/// `compiler_path`) is silently left out rather than failing the whole
/// lint — the engine's own diagnostics are still worth reporting either
/// way.
fn lint_with_compile_check(
    path: &Path,
    source: &str,
    config: &papyrus_lints::Config,
    function_table: &mut function_table::FunctionTable,
    additional_roots: &[String],
    compiler_path: &str,
    compile_check: bool,
) -> Vec<papyrus_lints::Diagnostic> {
    let mut diagnostics =
        papyrus_lints::lint_with_external_arguments(source, config, function_table);
    if config.rules.conflicting_script_versions {
        diagnostics.extend(script_locator::conflicting_script_versions(
            path,
            function_table.root(),
            function_table.additional_roots(),
        ));
    }

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
/// alongside `root`'s conventional source directories. `compiler_path` and
/// `compile_check` (see [`load_compiler_path`]/[`load_compile_check`])
/// control whether PapyrusCompiler.exe's own errors are merged in too —
/// see [`lint_with_compile_check`].
#[tauri::command]
fn lint_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = read_psc_source(Path::new(&path)).map_err(|err| err.to_string())?;
    let mut function_table = function_table::FunctionTable::new_with_additional_roots(
        PathBuf::from(root),
        additional_roots.clone(),
    );
    Ok(lint_with_compile_check(
        Path::new(&path),
        &source,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// the semicolon and indentation style `config` selects), writes the
/// repaired source back to disk, and returns the diagnostics that remain.
/// See [`lint_psc_file`] for `root`/`additional_roots`/`compiler_path`/
/// `compile_check`.
#[tauri::command]
fn repair_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let (source, encoding) =
        read_psc_source_with_encoding(Path::new(&path)).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair(&source, &config);
    if repaired != source {
        write_psc_source(Path::new(&path), &repaired, encoding).map_err(|err| err.to_string())?;
    }
    let mut function_table = function_table::FunctionTable::new_with_additional_roots(
        PathBuf::from(root),
        additional_roots.clone(),
    );
    Ok(lint_with_compile_check(
        Path::new(&path),
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Like [`repair_psc_file`], but applies only the automatic fix for `rule`
/// (a [`papyrus_lints::FIXABLE_RULE_IDS`] id), and restricts its effect to
/// `line` (1-indexed) — leaving every other line untouched — via
/// [`papyrus_lints::restrict_to_line`]. Drives the frontend's per-finding
/// "Fix this issue" button. Fails if the named rule's fix would change
/// `line`'s line count elsewhere in the file (e.g. `property-sorting`
/// relocating a property's declaration), since a single original line
/// number then no longer identifies the same line in the result; the
/// frontend surfaces that error and points the user at "Apply fixes"
/// instead.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn repair_psc_finding(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rule: String,
    line: usize,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let (source, encoding) =
        read_psc_source_with_encoding(Path::new(&path)).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair_filtered(&source, &config, Some(rule.as_str()));
    let repaired = papyrus_lints::restrict_to_line(&source, &repaired, line).ok_or_else(|| {
        "Fixing this issue would change other lines in the file; use \"Apply fixes\" instead."
            .to_string()
    })?;
    if repaired != source {
        write_psc_source(Path::new(&path), &repaired, encoding).map_err(|err| err.to_string())?;
    }
    let mut function_table = function_table::FunctionTable::new_with_additional_roots(
        PathBuf::from(root),
        additional_roots.clone(),
    );
    Ok(lint_with_compile_check(
        Path::new(&path),
        &repaired,
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
#[tauri::command]
fn list_script_members(
    root: String,
    type_name: String,
    additional_roots: Vec<String>,
) -> Vec<function_table::Member> {
    let mut function_table = function_table::FunctionTable::new_with_additional_roots(
        PathBuf::from(root),
        additional_roots,
    );
    function_table.list_members(&type_name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            parse_achlist_file,
            parse_papyrus_script,
            lint_papyrus_script,
            parse_psc_file,
            read_psc_file,
            write_psc_file,
            load_lint_config,
            save_lint_config,
            load_compiler_path,
            save_compiler_path,
            load_compile_check,
            save_compile_check,
            load_script_roots,
            load_project_info,
            save_script_roots,
            lint_psc_file,
            repair_psc_file,
            repair_psc_finding,
            compile_psc_file,
            list_script_members
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn get_app_version_returns_the_crate_version() {
        assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn source_commands_parse_and_lint_without_touching_disk() {
        let source = "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n";

        let script = parse_papyrus_script(source).expect("valid Papyrus should parse");
        assert_eq!(script.name, "Example");

        let diagnostics = lint_papyrus_script(source, papyrus_lints::Config::default());
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule == papyrus_lints::forbidden_functions::RULE && diagnostic.line == 4
        }));
    }

    #[test]
    fn psc_file_commands_round_trip_contents_and_parse_the_written_script() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        let initial = "ScriptName Initial\n";
        let replacement = "ScriptName Replacement\n";
        std::fs::write(&path, initial).unwrap();

        assert_eq!(
            read_psc_file(path.to_string_lossy().into_owned()).unwrap(),
            initial
        );
        write_psc_file(path.to_string_lossy().into_owned(), replacement.to_string()).unwrap();

        let script = parse_psc_file(path.to_string_lossy().into_owned()).unwrap();
        assert_eq!(script.name, "Replacement");
        assert_eq!(std::fs::read_to_string(path).unwrap(), replacement);
    }

    #[test]
    fn parse_psc_file_reflects_edits_made_between_calls_instead_of_a_stale_cache_entry() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        let path_string = path.to_string_lossy().into_owned();

        std::fs::write(&path, "ScriptName Initial\n").unwrap();
        let first = parse_psc_file(path_string.clone()).unwrap();
        assert_eq!(first.name, "Initial");

        std::fs::write(&path, "ScriptName Changed\n").unwrap();
        let second = parse_psc_file(path_string).unwrap();
        assert_eq!(second.name, "Changed");
    }

    #[test]
    fn parse_psc_file_reuses_a_valid_cached_script() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Cached.psc");
        let path_string = path.to_string_lossy().into_owned();
        std::fs::write(&path, "ScriptName Cached\n").unwrap();

        assert_eq!(parse_psc_file(path_string.clone()).unwrap().name, "Cached");
        assert_eq!(parse_psc_file(path_string).unwrap().name, "Cached");
    }

    #[test]
    fn file_commands_report_io_errors_instead_of_panicking() {
        let missing = tempdir().unwrap().path().join("missing.psc");
        let path = missing.to_string_lossy().into_owned();

        assert!(read_psc_file(path.clone()).is_err());
        assert!(write_psc_file(path.clone(), "ScriptName Example\n".to_string()).is_err());
        assert!(parse_psc_file(path.clone()).is_err());
        assert!(lint_psc_file(
            path.clone(),
            missing.parent().unwrap().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            String::new(),
            false,
        )
        .is_err());
        assert!(repair_psc_file(
            path,
            missing.parent().unwrap().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            String::new(),
            false,
        )
        .is_err());
    }

    #[test]
    fn repair_psc_file_persists_fixes_and_returns_only_remaining_findings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(
            &path,
            "ScriptName Example  \n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n",
        )
        .unwrap();

        let diagnostics = repair_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::trailing_whitespace::RULE));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.rule == papyrus_lints::forbidden_functions::RULE }));
    }

    #[test]
    fn repair_psc_finding_fixes_only_the_named_rule_on_the_given_line() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(
            &path,
            "ScriptName Example  \n\nFunction Run(Int left,Int right)  \nEndFunction\n",
        )
        .unwrap();

        let diagnostics = repair_psc_finding(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::comma_spacing::RULE.to_string(),
            3,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example  \n\nFunction Run(Int left, Int right)  \nEndFunction\n"
        );
        assert!(diagnostics.iter().all(|diagnostic| !(diagnostic.line == 3
            && diagnostic.rule == papyrus_lints::comma_spacing::RULE)));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.line == 1 && diagnostic.rule == papyrus_lints::trailing_whitespace::RULE
        }));
    }

    #[test]
    fn repair_psc_finding_rejects_a_fix_that_would_change_the_line_count() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        let source = "ScriptName Example\n\nInt Property zulu Auto\nInt Property alpha Auto\n";
        std::fs::write(&path, source).unwrap();
        let mut config = papyrus_lints::Config::default();
        config.rules.property_sorting = true;

        let error = repair_psc_finding(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::property_sorting::RULE.to_string(),
            4,
        )
        .unwrap_err();

        assert!(error.contains("Apply fixes"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    }

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
    fn achlist_command_resolves_entries_and_reports_invalid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("scripts.achlist");
        std::fs::write(&path, r#"["scripts/source/Example.psc"]"#).unwrap();

        assert_eq!(
            parse_achlist_file(path.to_string_lossy().into_owned()).unwrap(),
            vec![dir
                .path()
                .join("scripts/source/Example.psc")
                .to_string_lossy()
                .into_owned()]
        );

        std::fs::write(&path, "not json").unwrap();
        assert!(parse_achlist_file(path.to_string_lossy().into_owned()).is_err());

        let missing = dir.path().join("missing.achlist");
        assert!(parse_achlist_file(missing.to_string_lossy().into_owned()).is_err());
    }

    #[test]
    fn parse_commands_report_invalid_papyrus() {
        let invalid = "Function MissingScriptName()\nEndFunction\n";
        assert!(parse_papyrus_script(invalid).is_err());

        let dir = tempdir().unwrap();
        let path = dir.path().join("Invalid.psc");
        std::fs::write(&path, invalid).unwrap();
        assert!(parse_psc_file(path.to_string_lossy().into_owned()).is_err());
    }

    #[test]
    fn config_commands_round_trip_lint_and_compiler_settings() {
        let dir = tempdir().unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        assert_eq!(
            load_lint_config(dir_string.clone()).unwrap(),
            Default::default()
        );
        save_lint_config(dir_string.clone(), config.clone()).unwrap();
        assert_eq!(load_lint_config(dir_string.clone()).unwrap(), config);

        save_compiler_path(dir_string.clone(), "  /tools/compiler  ".to_string()).unwrap();
        assert_eq!(
            load_compiler_path(dir_string.clone()).unwrap(),
            Some("/tools/compiler".to_string())
        );
        save_compiler_path(dir_string.clone(), " \t ".to_string()).unwrap();
        assert_eq!(load_compiler_path(dir_string.clone()).unwrap(), None);

        assert!(!load_compile_check(dir_string.clone()).unwrap());
        save_compile_check(dir_string.clone(), true).unwrap();
        assert!(load_compile_check(dir_string.clone()).unwrap());
        save_compile_check(dir_string.clone(), false).unwrap();
        assert!(!load_compile_check(dir_string.clone()).unwrap());

        assert_eq!(
            load_script_roots(dir_string.clone()).unwrap(),
            Vec::<String>::new()
        );
        save_script_roots(dir_string.clone(), vec!["../SharedScripts".to_string()]).unwrap();
        assert_eq!(
            load_script_roots(dir_string).unwrap(),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn project_info_reports_detected_roots_and_configuration_file() {
        let dir = tempdir().unwrap();
        let scripts = dir.path().join("scripts/source");
        let shared = dir.path().join("shared");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            dir.path().join("papyrus-lint.yml"),
            "additional_script_roots:\n  - shared\n",
        )
        .unwrap();

        let info = load_project_info(dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(
            info,
            ProjectInfo {
                detected_script_roots: vec![
                    scripts.to_string_lossy().into_owned(),
                    shared.to_string_lossy().into_owned(),
                ],
                used_configuration_file: Some(
                    dir.path()
                        .join("papyrus-lint.yml")
                        .to_string_lossy()
                        .into_owned()
                ),
            }
        );
    }

    #[test]
    fn config_commands_report_invalid_yaml_and_unwritable_directories() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("papyrus-lint.yaml"), "semicolon: [").unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();

        assert!(load_lint_config(dir_string.clone()).is_err());
        assert!(load_compiler_path(dir_string.clone()).is_err());
        assert!(save_compiler_path(dir_string.clone(), "compiler".to_string()).is_err());
        assert!(load_compile_check(dir_string.clone()).is_err());
        assert!(save_compile_check(dir_string.clone(), true).is_err());
        assert!(load_script_roots(dir_string.clone()).is_err());
        assert!(
            save_script_roots(dir_string.clone(), vec!["../SharedScripts".to_string()]).is_err()
        );
        assert!(save_lint_config(dir_string, Default::default()).is_err());

        let missing_dir = dir.path().join("missing").to_string_lossy().into_owned();
        assert!(save_lint_config(missing_dir, Default::default()).is_err());
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
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == papyrus_lints::forbidden_functions::RULE));
    }

    #[test]
    fn repair_does_not_rewrite_an_already_clean_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&path, source).unwrap();

        let diagnostics = repair_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    }

    #[test]
    fn repair_psc_file_preserves_a_cp1252_encoded_files_encoding() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        // "ScriptName Example  \n\n; caf\xE9\n" with trailing whitespace on
        // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
        // file as a whole invalid UTF-8.
        let mut contents = b"ScriptName Example  \n\n; caf".to_vec();
        contents.push(0xE9);
        contents.push(b'\n');
        std::fs::write(&path, &contents).unwrap();

        repair_psc_file(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        let mut expected = b"ScriptName Example\n\n; caf".to_vec();
        expected.push(0xE9);
        expected.push(b'\n');
        assert_eq!(std::fs::read(&path).unwrap(), expected);
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
            "   ".to_string(),
            true,
        )
        .unwrap();

        assert!(diagnostics.is_empty());
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
        );

        assert_eq!(members.len(), 1);
    }

    #[test]
    fn list_script_members_is_empty_for_an_unresolvable_type() {
        let dir = tempdir().unwrap();

        assert!(list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Missing".to_string(),
            Vec::new(),
        )
        .is_empty());
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
}

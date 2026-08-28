pub mod compiler;
pub mod pex_header;

use std::path::{Path, PathBuf};

use papyrus_lint_core::{achlist, config, function_table};

/// Returns the desktop app's version (from `src-tauri/Cargo.toml`, kept in
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

/// Reads the `.psc` file at `path` and parses it into a `Script` AST.
#[tauri::command]
fn parse_psc_file(path: String) -> Result<papyrus_parser::ast::Script, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    papyrus_parser::parse(&source).map_err(|err| err.to_string())
}

/// Reads the `.psc` file at `path` and returns its raw source text, for the
/// frontend's syntax-highlighted code viewer.
#[tauri::command]
fn read_psc_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|err| err.to_string())
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

/// Compiles the `.psc` file at `path` using the compiler executable at
/// `compiler_path` (see [`load_compiler_path`]/[`resolve_compiler_path`]
/// for how the frontend obtains that path). Returns an error if
/// `compiler_path` is blank/unconfigured or the compiler process itself
/// couldn't be run; a script that fails to compile is still reported as
/// `Ok`, with [`compiler::CompileOutcome::success`] false and the
/// compiler's stdout/stderr carrying the reported errors.
#[tauri::command]
fn compile_psc_file(
    path: String,
    compiler_path: String,
) -> Result<compiler::CompileOutcome, String> {
    let compiler_path = compiler_path.trim();
    if compiler_path.is_empty() {
        return Err(
            "No PapyrusCompiler.exe path is configured. Set one in the Settings tab.".to_string(),
        );
    }

    compiler::compile_psc_file(Path::new(compiler_path), &PathBuf::from(path))
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring the semicolon style `config` selects. `root` is the project
/// root (conventionally the directory containing the `.achlist` file); it
/// lets the "Argument type check" lint resolve calls to functions declared
/// on other scripts under `root`, and lets the "Return type check" lint
/// accept a returned value whose script under `root` extends the declared
/// return type.
#[tauri::command]
fn lint_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_external_arguments(
        &source,
        &config,
        &mut function_table,
    ))
}

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// the semicolon and indentation style `config` selects), writes the
/// repaired source back to disk, and returns the diagnostics that remain.
/// See [`lint_psc_file`] for `root`.
#[tauri::command]
fn repair_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let source = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair(&source, &config);
    if repaired != source {
        std::fs::write(&path, &repaired).map_err(|err| err.to_string())?;
    }
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
    Ok(papyrus_lints::lint_with_external_arguments(
        &repaired,
        &config,
        &mut function_table,
    ))
}

/// Lists every function and property available on an object of type
/// `type_name` (including those inherited via `Extends`), for driving the
/// code viewer's editor autocompletion. See [`lint_psc_file`] for `root`.
#[tauri::command]
fn list_script_members(root: String, type_name: String) -> Vec<function_table::Member> {
    let mut function_table = function_table::FunctionTable::new(PathBuf::from(root));
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
            lint_psc_file,
            repair_psc_file,
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
    fn file_commands_report_io_errors_instead_of_panicking() {
        let missing = tempdir().unwrap().path().join("missing.psc");
        let path = missing.to_string_lossy().into_owned();

        assert!(read_psc_file(path.clone()).is_err());
        assert!(parse_psc_file(path.clone()).is_err());
        assert!(lint_psc_file(
            path.clone(),
            missing.parent().unwrap().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
        )
        .is_err());
        assert!(repair_psc_file(
            path,
            missing.parent().unwrap().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
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
    fn compile_psc_file_rejects_a_blank_compiler_path_before_spawning() {
        assert!(compile_psc_file("Example.psc".to_string(), "  \t".to_string()).is_err());
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
        assert_eq!(load_compiler_path(dir_string).unwrap(), None);
    }

    #[test]
    fn config_commands_report_invalid_yaml_and_unwritable_directories() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("papyrus-lint.yaml"), "semicolon: [").unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();

        assert!(load_lint_config(dir_string.clone()).is_err());
        assert!(load_compiler_path(dir_string.clone()).is_err());
        assert!(save_compiler_path(dir_string.clone(), "compiler".to_string()).is_err());
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
        )
        .unwrap();

        assert!(diagnostics.is_empty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
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
        );

        let names: std::collections::HashSet<_> =
            members.iter().map(function_table::Member::name).collect();
        assert_eq!(
            names,
            std::collections::HashSet::from(["DoThing", "IsAwesome"])
        );
    }

    #[test]
    fn list_script_members_is_empty_for_an_unresolvable_type() {
        let dir = tempdir().unwrap();

        assert!(list_script_members(
            dir.path().to_string_lossy().into_owned(),
            "Missing".to_string()
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
        )
        .unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.stdout, "command wrapper\n");
    }
}

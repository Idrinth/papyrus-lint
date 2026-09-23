use super::*;
use tempfile::tempdir;

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
        load_script_roots(dir_string.clone()).unwrap(),
        vec!["../SharedScripts".to_string()]
    );

    assert_eq!(
        load_lookup_script_roots(dir_string.clone()).unwrap(),
        Vec::<String>::new()
    );
    save_lookup_script_roots(
        dir_string.clone(),
        vec!["  ../BaseScripts  ".to_string(), "   ".to_string()],
    )
    .unwrap();
    assert_eq!(
        load_lookup_script_roots(dir_string).unwrap(),
        vec!["../BaseScripts".to_string()]
    );
}

#[test]
fn lint_config_from_path_commands_round_trip_regardless_of_project_directory() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("custom-config.yaml");
    let path_string = path.to_string_lossy().into_owned();
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation_width: 2,
        ..papyrus_lints::Config::default()
    };

    assert!(load_lint_config_from_path(path_string.clone()).is_err());
    save_lint_config_to_path(path_string.clone(), config.clone()).unwrap();
    assert_eq!(load_lint_config_from_path(path_string).unwrap(), config);
}

#[test]
fn lint_config_from_path_commands_report_parse_and_write_errors() {
    let dir = tempdir().unwrap();
    let invalid = dir.path().join("invalid.yaml");
    std::fs::write(&invalid, "semicolon: [").unwrap();

    assert!(load_lint_config_from_path(invalid.to_string_lossy().into_owned()).is_err());
    assert!(save_lint_config_to_path(
        dir.path()
            .join("missing/config.yaml")
            .to_string_lossy()
            .into_owned(),
        Default::default(),
    )
    .is_err());
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
fn project_info_is_empty_when_the_project_has_no_known_layout_or_config() {
    let dir = tempdir().unwrap();

    let info = load_project_info(dir.path().to_string_lossy().into_owned()).unwrap();

    assert_eq!(
        info,
        ProjectInfo {
            detected_script_roots: Vec::new(),
            used_configuration_file: None,
        }
    );
}

#[test]
fn project_info_reports_an_invalid_project_configuration() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("papyrus-lint.yaml"), "semicolon: [").unwrap();

    let error = load_project_info(dir.path().to_string_lossy().into_owned())
        .expect_err("invalid project configuration should be reported");

    assert!(error.contains("papyrus-lint.yaml"));
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
    assert!(save_script_roots(dir_string.clone(), vec!["../SharedScripts".to_string()]).is_err());
    assert!(load_lookup_script_roots(dir_string.clone()).is_err());
    assert!(
        save_lookup_script_roots(dir_string.clone(), vec!["../BaseScripts".to_string()]).is_err()
    );
    assert!(save_lint_config(dir_string, Default::default()).is_err());

    let missing_dir = dir.path().join("missing").to_string_lossy().into_owned();
    assert!(save_lint_config(missing_dir, Default::default()).is_err());
}

#[test]
fn load_compiler_path_auto_detects_an_adjacent_compiler_executable() {
    let root = tempdir().unwrap();
    let compiler_dir = root.path().join("Papyrus Compiler");
    std::fs::create_dir(&compiler_dir).unwrap();
    let compiler = compiler_dir.join("PapyrusCompiler.exe");
    std::fs::write(&compiler, b"").unwrap();
    let data_dir = root.path().join("Data");
    std::fs::create_dir(&data_dir).unwrap();

    assert_eq!(
        load_compiler_path(data_dir.to_string_lossy().into_owned()).unwrap(),
        Some(compiler.to_string_lossy().into_owned())
    );
}

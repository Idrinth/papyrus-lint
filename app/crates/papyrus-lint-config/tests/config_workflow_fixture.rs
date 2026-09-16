//! Public-API integration coverage for updating the non-lint settings in a
//! project configuration. The desktop app saves these settings independently,
//! so every update must preserve the values written by the preceding updates.

use std::fs;

use papyrus_lint_config::{
    config_file_path, load_compile_check, load_compiler_path, load_lookup_script_roots,
    load_script_roots, load_strict_achlist_scope, save_compile_check, save_compiler_path,
    save_lookup_script_roots, save_script_roots,
};

#[test]
fn independently_saved_project_settings_survive_a_complete_editing_workflow() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let config_path = project.path().join("papyrus-lint.yaml");
    fs::write(&config_path, "strict_achlist_scope: true\n").expect("failed to seed project config");

    save_compiler_path(project.path(), Some("  C:/Tools/PapyrusCompiler.exe  "))
        .expect("compiler path should save");
    save_compile_check(project.path(), true).expect("compile check should save");
    save_script_roots(
        project.path(),
        &[
            "  imports/one  ".to_string(),
            "  ".to_string(),
            "imports/two".to_string(),
        ],
    )
    .expect("script roots should save");
    save_lookup_script_roots(
        project.path(),
        &[
            "  C:/Skyrim/Data/Scripts/Source  ".to_string(),
            "  ".to_string(),
            "C:/Skyrim/Data/Source/Scripts".to_string(),
        ],
    )
    .expect("lookup script roots should save");

    assert_eq!(config_file_path(project.path()), Some(config_path.clone()));
    assert_eq!(
        load_compiler_path(project.path()).expect("compiler path should load"),
        Some("C:/Tools/PapyrusCompiler.exe".to_string())
    );
    assert!(load_compile_check(project.path()).expect("compile check should load"));
    assert!(load_strict_achlist_scope(project.path()).expect("scope should load"));
    assert_eq!(
        load_script_roots(project.path()).expect("script roots should load"),
        vec!["imports/one", "imports/two"]
    );
    assert_eq!(
        load_lookup_script_roots(project.path()).expect("lookup script roots should load"),
        vec![
            "C:/Skyrim/Data/Scripts/Source",
            "C:/Skyrim/Data/Source/Scripts"
        ]
    );

    let saved = fs::read_to_string(config_path).expect("saved config should be readable");
    assert!(saved.contains("compiler_path: C:/Tools/PapyrusCompiler.exe"));
    assert!(saved.contains("compile_check: true"));
    assert!(saved.contains("strict_achlist_scope: true"));
    assert!(saved.contains("lookup_script_roots:"));
}

#[test]
fn clearing_the_compiler_override_preserves_the_other_project_settings() {
    let project = tempfile::tempdir().expect("failed to create project directory");

    save_compiler_path(project.path(), Some("C:/Tools/PapyrusCompiler.exe"))
        .expect("compiler path should save");
    save_compile_check(project.path(), true).expect("compile check should save");
    save_script_roots(project.path(), &["imports".to_string()]).expect("script roots should save");
    save_lookup_script_roots(
        project.path(),
        &["C:/Skyrim/Data/Scripts/Source".to_string()],
    )
    .expect("lookup script roots should save");
    save_compiler_path(project.path(), Some("   ")).expect("compiler path should clear");

    assert_eq!(
        load_compiler_path(project.path()).expect("compiler path should load"),
        None
    );
    assert!(load_compile_check(project.path()).expect("compile check should load"));
    assert_eq!(
        load_script_roots(project.path()).expect("script roots should load"),
        vec!["imports"]
    );
    assert_eq!(
        load_lookup_script_roots(project.path()).expect("lookup script roots should load"),
        vec!["C:/Skyrim/Data/Scripts/Source"]
    );
}

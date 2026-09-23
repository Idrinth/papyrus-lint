use std::fs;

use super::*;

#[test]
fn script_lookup_dirs_for_skyrim_install_returns_existing_source_directories() {
    let install = tempfile::tempdir().expect("failed to create temp dir");
    let scripts_source = install.path().join("Data/Scripts/Source");
    let source_scripts = install.path().join("Data/Source/Scripts");
    fs::create_dir_all(&scripts_source).expect("failed to create Scripts/Source");
    fs::create_dir_all(&source_scripts).expect("failed to create Source/Scripts");

    let dirs = script_lookup_dirs_for_skyrim_install(install.path());

    assert_eq!(
        dirs,
        vec![
            scripts_source.to_string_lossy().into_owned(),
            source_scripts.to_string_lossy().into_owned()
        ]
    );
}

#[test]
fn script_lookup_dirs_for_skyrim_install_omits_missing_directories() {
    let install = tempfile::tempdir().expect("failed to create temp dir");
    let scripts_source = install.path().join("Data/Scripts/Source");
    fs::create_dir_all(&scripts_source).expect("failed to create Scripts/Source");

    let dirs = script_lookup_dirs_for_skyrim_install(install.path());

    assert_eq!(dirs, vec![scripts_source.to_string_lossy().into_owned()]);
}

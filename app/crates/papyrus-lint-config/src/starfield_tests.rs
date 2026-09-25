use std::fs;

use super::*;

#[test]
fn script_lookup_dirs_for_starfield_install_returns_existing_source_directories() {
    let install = tempfile::tempdir().expect("failed to create temp dir");
    let source = install.path().join("Data/Scripts/Source");
    let base = install.path().join("Data/Scripts/Source/Base");
    let user = install.path().join("Data/Scripts/Source/User");
    fs::create_dir_all(&source).expect("failed to create Source");
    fs::create_dir_all(&base).expect("failed to create Source/Base");
    fs::create_dir_all(&user).expect("failed to create Source/User");

    let dirs = script_lookup_dirs_for_starfield_install(install.path());

    assert_eq!(
        dirs,
        vec![
            source.to_string_lossy().into_owned(),
            base.to_string_lossy().into_owned(),
            user.to_string_lossy().into_owned()
        ]
    );
}

#[test]
fn script_lookup_dirs_for_starfield_install_omits_missing_directories() {
    let install = tempfile::tempdir().expect("failed to create temp dir");
    let source = install.path().join("Data/Scripts/Source");
    fs::create_dir_all(&source).expect("failed to create Source");

    let dirs = script_lookup_dirs_for_starfield_install(install.path());

    assert_eq!(dirs, vec![source.to_string_lossy().into_owned()]);
}

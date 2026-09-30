use super::*;

use std::fs;
use std::path::{Path, PathBuf};

fn write_file(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "").expect("failed to write test script file");
    path
}

#[test]
fn uses_conventional_tree_not_nested_parent() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source = root.path().join("Scripts/Source/DLC03/Fragments/Quests");
    fs::create_dir_all(&source).expect("failed to create namespaced dir");
    let script = write_file(&source, "QF_Example.psc");

    assert_eq!(
        inferred_script_search_root(&script, root.path()),
        root.path().join("Scripts/Source")
    );
}

#[test]
fn keeps_flat_custom_parent() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let custom = root.path().join("mods/MyMod");
    fs::create_dir_all(&custom).expect("failed to create custom dir");
    let script = write_file(&custom, "MyQuest.psc");

    assert_eq!(inferred_script_search_root(&script, root.path()), custom);
}

#[test]
fn accepts_source_scripts_casing() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let nested = root.path().join("Source/Scripts/User");
    fs::create_dir_all(&nested).expect("failed to create namespace dir");
    let script = write_file(&nested, "Foo.psc");

    assert_eq!(
        inferred_script_search_root(&script, root.path()),
        root.path().join("Source/Scripts")
    );
}

#[test]
fn does_not_search_above_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let project = root.path().join("Scripts/Source/project");
    let nested = project.join("custom/nested");
    fs::create_dir_all(&nested).expect("failed to create custom dir");
    let script = write_file(&nested, "Foo.psc");

    assert_eq!(inferred_script_search_root(&script, &project), nested);
}

#[test]
fn falls_back_to_parent_when_project_root_is_not_an_ancestor() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let custom = root.path().join("unrelated/custom");
    fs::create_dir_all(&custom).expect("failed to create custom dir");
    let script = write_file(&custom, "Foo.psc");

    assert_eq!(
        inferred_script_search_root(&script, &root.path().join("project")),
        custom
    );
}

#[test]
fn parentless_path_uses_project_root() {
    let project = PathBuf::from("project");

    assert_eq!(
        inferred_script_search_root(std::path::Path::new("/"), &project),
        project
    );
}

#[test]
fn conventional_root_requires_matching_adjacent_directory_names() {
    assert!(is_conventional_script_root(std::path::Path::new(
        "Data/SCRIPTS/sOuRcE"
    )));
    assert!(is_conventional_script_root(std::path::Path::new(
        "Data/SOURCE/sCrIpTs"
    )));
    assert!(!is_conventional_script_root(std::path::Path::new(
        "Data/assets/source"
    )));
    assert!(!is_conventional_script_root(std::path::Path::new(
        "Data/source/assets"
    )));
    assert!(!is_conventional_script_root(std::path::Path::new("source")));
    assert!(!is_conventional_script_root(std::path::Path::new("/")));
}

#[cfg(unix)]
#[test]
fn non_utf8_directory_names_are_not_conventional_roots() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let non_utf8 = OsStr::from_bytes(b"\xff");

    assert!(!is_conventional_script_root(
        &PathBuf::from("scripts").join(non_utf8)
    ));
    assert!(!is_conventional_script_root(
        &PathBuf::from(non_utf8).join("source")
    ));
}

#[test]
fn relative_path_keeps_namespace_folder_under_cased_scripts_source() {
    let relative = strip_prefix_ignore_ascii_case(
        Path::new("/game/Scripts/Source/NativeTerminal/ContainerScript.psc"),
        Path::new("/game/scripts/source"),
    )
    .expect("cased Scripts/Source should still strip");
    assert_eq!(relative, Path::new("NativeTerminal/ContainerScript.psc"));
}

#[test]
fn relative_path_from_inferred_root_keeps_native_terminal_namespace() {
    let relative = relative_path_from_inferred_root(
        Path::new("/mods/StarfieldBaseScripts/Scripts/Source/NativeTerminal/ContainerScript.psc"),
        Path::new("/mods/StarfieldBaseScripts"),
    )
    .expect("conventional Scripts/Source ancestor should be used");
    assert_eq!(relative, Path::new("NativeTerminal/ContainerScript.psc"));
}

#[test]
fn relative_path_from_inferred_root_ignores_flat_custom_folders() {
    assert!(relative_path_from_inferred_root(
        Path::new("/mods/MyMod/MyQuest.psc"),
        Path::new("/mods/MyMod"),
    )
    .is_none());
}

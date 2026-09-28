use super::*;

use std::fs;
use std::path::PathBuf;

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

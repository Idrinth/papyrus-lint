use super::super::super::test_support::write_script;
use super::super::*;

#[test]
fn event_index_grafts_a_child_onto_an_indexed_bundled_ancestor() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ChildScript",
        "ScriptName ChildScript Extends Form\n\nEvent OnChildReady()\nEndEvent\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    // Index the ancestor first so building ChildScript takes the cached-ancestor
    // path. Form comes from the bundled scripts and therefore has no file mtime.
    assert_eq!(table.has_event("Form", "OnUpdate"), Some(true));
    assert_eq!(table.has_event("ChildScript", "OnUpdate"), Some(true));
    assert_eq!(table.has_event("ChildScript", "OnChildReady"), Some(true));
    assert_eq!(table.has_event("ChildScript", "OnMissing"), Some(false));
    assert!(matches!(
        table.has_event_cached("childscript", "onupdate"),
        CacheProbe::Hit(Some(true))
    ));
}

#[test]
fn event_index_is_discarded_when_an_ancestor_is_removed() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nEvent OnReady()\nEndEvent\n",
    );
    write_script(
        root.path(),
        "ChildScript",
        "ScriptName ChildScript Extends ParentScript\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert_eq!(table.has_event("ChildScript", "OnReady"), Some(true));

    std::fs::remove_file(root.path().join("scripts/source/ParentScript.psc"))
        .expect("failed to remove parent script");

    assert!(matches!(
        table.has_event_cached("ChildScript", "OnReady"),
        CacheProbe::Miss
    ));
    assert_eq!(table.has_event("ChildScript", "OnReady"), None);
}

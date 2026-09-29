use super::super::super::test_support::write_script;
use super::super::*;

#[test]
fn registers_remote_event_true_for_literal_on_same_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n",
    );

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("Foo", "OnCellAttach"),
        Some(true)
    );
    assert_eq!(
        table.registers_remote_event("foo", "oncellattach"),
        Some(true)
    );
    assert_eq!(table.registers_remote_event("Foo", "OnDeath"), Some(false));
}

#[test]
fn registers_remote_event_true_when_parent_registers() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends ParentScript\n",
    );

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("Child", "OnCellAttach"),
        Some(true)
    );
}

#[test]
fn registers_remote_event_false_when_ancestry_has_no_registration() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nEvent OnInit()\nEndEvent\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends ParentScript\n",
    );

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("Child", "OnCellAttach"),
        Some(false)
    );
}

#[test]
fn registers_remote_event_true_when_parent_registration_is_opaque() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nFunction Start(ScriptEventName eventName)\n    RegisterForRemoteEvent(akTarget, eventName)\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends ParentScript\n",
    );

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("Child", "OnCellAttach"),
        Some(true)
    );
}

#[test]
fn registers_remote_event_none_when_parent_is_missing() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends MissingParent\n",
    );

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(table.registers_remote_event("Child", "OnCellAttach"), None);
}

#[test]
fn registers_remote_event_none_for_a_circular_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "First", "ScriptName First Extends Second\n");
    write_script(root.path(), "Second", "ScriptName Second Extends First\n");

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(table.registers_remote_event("First", "OnCellAttach"), None);
}

#[test]
fn cached_remote_event_lookup_reports_a_miss_before_loading() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let table = FunctionTable::new(root.path().to_path_buf());

    assert!(matches!(
        table.registers_remote_event_cached("Foo", "OnCellAttach"),
        CacheProbe::Miss
    ));
}

#[test]
fn cached_remote_event_lookup_walks_loaded_ancestry() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends PARENTSCRIPT\n",
    );
    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("Child", "OnCellAttach"),
        Some(true)
    );
    assert!(matches!(
        table.registers_remote_event_cached("CHILD", "oncellattach"),
        CacheProbe::Hit(Some(true))
    ));
    assert!(matches!(
        table.registers_remote_event_cached("child", "OnDeath"),
        CacheProbe::Hit(Some(false))
    ));
}

#[test]
fn cached_remote_event_lookup_handles_unresolvable_and_circular_ancestry() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MissingChild",
        "ScriptName MissingChild Extends Missing\n",
    );
    write_script(root.path(), "First", "ScriptName First Extends Second\n");
    write_script(root.path(), "Second", "ScriptName Second Extends First\n");
    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_game(papyrus_lints::Game::Fallout4);

    assert_eq!(
        table.registers_remote_event("MissingChild", "OnCellAttach"),
        None
    );
    assert_eq!(table.registers_remote_event("First", "OnCellAttach"), None);

    assert!(matches!(
        table.registers_remote_event_cached("MissingChild", "OnCellAttach"),
        CacheProbe::Hit(None)
    ));
    assert!(matches!(
        table.registers_remote_event_cached("First", "OnCellAttach"),
        CacheProbe::Hit(None)
    ));
}

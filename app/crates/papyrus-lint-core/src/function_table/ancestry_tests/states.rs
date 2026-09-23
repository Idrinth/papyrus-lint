use super::super::super::test_support::write_script;
use super::super::*;

#[test]
fn has_state_true_for_a_state_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_state("Foo", "Active"));
    assert!(table.has_state("foo", "active"));
}

#[test]
fn has_state_true_for_a_state_declared_on_an_ancestor() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nState Active\nEndState\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_state("Child", "Active"));
}

#[test]
fn has_state_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_state("Foo", "DoesNotExist"));
    assert!(!table.has_state("Missing", "Anything"));
}

#[test]
fn has_state_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_state("A", "Anything"));
}

#[test]
fn ancestor_states_includes_the_types_own_and_inherited_states() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nAuto State Idle\nEndState\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let mut states = table.ancestor_states("Child");
    states.sort();

    assert_eq!(
        states,
        vec![("active".to_string(), false), ("idle".to_string(), true)]
    );
}

#[test]
fn ancestor_states_is_empty_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestor_states("Missing").is_empty());
}

#[test]
fn ancestor_states_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "A",
        "ScriptName A Extends B\n\nState FromA\nEndState\n",
    );
    write_script(
        root.path(),
        "B",
        "ScriptName B Extends A\n\nState FromB\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let mut states = table.ancestor_states("A");
    states.sort();

    assert_eq!(
        states,
        vec![("froma".to_string(), false), ("fromb".to_string(), false)]
    );
}

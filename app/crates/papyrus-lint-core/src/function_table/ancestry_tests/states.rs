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

#[test]
fn descendant_goto_state_counts_as_a_use_of_an_ancestor_state() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nState Busy\nEndState\n\nState Unused\nEndState\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nFunction Demo()\n    GoToState(\"Busy\")\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Unrelated",
        "ScriptName Unrelated\n\nFunction Demo()\n    GoToState(\"Unused\")\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.descendant_targets_state("Base", "Busy"));
    assert!(table.descendant_targets_state("base", "busy"));
    assert!(!table.descendant_targets_state("Base", "Unused"));
    assert!(!table.descendant_targets_state("Child", "Busy"));
}

#[test]
fn descendant_goto_state_walks_through_an_intermediate_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nState Busy\nEndState\n",
    );
    write_script(root.path(), "Middle", "ScriptName Middle Extends Base\n");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Middle\n\nFunction Demo()\n    self.GoToState(\"Busy\")\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.descendant_targets_state("Base", "Busy"));
    assert!(!table.descendant_targets_state("Middle", "Busy"));
}

#[test]
fn descendant_targets_state_honors_known_scripts_and_does_not_loop_on_cycles() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let base = root.path().join("Base.psc");
    let child = root.path().join("Child.psc");
    let other = root.path().join("Other.psc");
    std::fs::write(
        &base,
        "ScriptName Base Extends Child\n\nState Busy\nEndState\n",
    )
    .expect("failed to write base");
    std::fs::write(
        &child,
        "ScriptName Child Extends Base\n\nFunction Demo()\n    GoToState(\"Busy\")\nEndFunction\n",
    )
    .expect("failed to write child");
    std::fs::write(&other, "ScriptName Other\n\nState Busy\nEndState\n")
        .expect("failed to write other");

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[base, child]);

    assert!(table.descendant_targets_state("Base", "Busy"));
    assert!(!table.descendant_targets_state("Other", "Busy"));
}

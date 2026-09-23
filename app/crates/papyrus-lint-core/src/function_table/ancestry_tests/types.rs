use super::super::super::test_support::write_script;
use super::super::*;

#[test]
fn is_subtype_true_for_direct_and_transitive_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");
    write_script(
        root.path(),
        "ClothingArmor",
        "ScriptName ClothingArmor Extends Armor\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("Armor", "Form"));
    assert!(table.is_subtype("ClothingArmor", "Form"));
    assert!(table.is_subtype("armor", "form"));
    assert!(table.is_subtype("Form", "Form"));
}

#[test]
fn is_subtype_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Weapon", "ScriptName Weapon Extends Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.is_subtype("Form", "Weapon"));
    assert!(!table.is_subtype("Weapon", "Armor"));
    assert!(!table.is_subtype("Missing", "Form"));
}

#[test]
fn is_subtype_resolves_bundled_vanilla_types_with_no_project_script() {
    // Regression for https://github.com/Idrinth/papyrus-lint/issues/1082:
    // none of `Actor`/`ObjectReference`/`Form`/`Spell` have a `.psc` under
    // `root` (typical for a mod project), so this can only pass via the
    // bundled vanilla AST cache's name index.
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("Actor", "ObjectReference"));
    assert!(table.is_subtype("Actor", "Form"));
    assert!(table.is_subtype("Spell", "Form"));
    assert!(!table.is_subtype("Form", "Actor"));
    assert!(!table.is_subtype("Spell", "ObjectReference"));
}

#[test]
fn is_subtype_falls_back_to_bundled_vanilla_types_past_a_project_scripts_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MyQuestScript",
        "ScriptName MyQuestScript Extends Quest\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("MyQuestScript", "Quest"));
    assert!(table.is_subtype("MyQuestScript", "Form"));
}

#[test]
fn is_subtype_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.is_subtype("A", "SomethingElse"));
}

#[test]
fn ancestry_fully_known_true_for_a_script_with_no_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("Form"));
}

#[test]
fn ancestry_fully_known_true_for_a_project_chain_ending_in_a_bundled_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MyQuestScript",
        "ScriptName MyQuestScript Extends Quest\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("MyQuestScript"));
}

#[test]
fn ancestry_fully_known_true_for_bundled_vanilla_types_with_no_project_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("Armor"));
    assert!(table.ancestry_fully_known("Weapon"));
    assert!(table.ancestry_fully_known("Actor"));
    assert!(table.ancestry_fully_known("Form"));
}

#[test]
fn ancestry_fully_known_false_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("SomeModsQuestScript"));
}

#[test]
fn ancestry_fully_known_false_when_a_project_script_extends_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends SomeModsQuestScript\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("Child"));
}

#[test]
fn ancestry_fully_known_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("A"));
}

use super::super::test_support::write_script;
use super::*;
use papyrus_lints::ExternalSignatures;
use std::sync::Mutex;

#[test]
fn shared_function_table_forwards_every_external_signature_lookup() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run() Global\nEndFunction\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Helpers\n");
    write_script(
        root.path(),
        "Properties",
        "ScriptName Properties\n\nString Property Name Auto\nInt Age = 1\n",
    );
    write_script(
        root.path(),
        "States",
        "ScriptName States\n\nState Active\nEndState\n",
    );
    let table = Mutex::new(FunctionTable::new(root.path().to_path_buf()));
    let mut shared = SharedFunctionTable(&table);

    let params = shared
        .lookup("Helpers", "Run")
        .expect("function should resolve through the adapter");
    assert!(params.is_empty());
    assert!(shared.is_subtype("Child", "Helpers"));
    assert!(shared.has_property("Properties", "Name"));
    assert!(shared.has_field("Properties", "Age"));
    assert_eq!(shared.property_types("Properties"), vec!["String"]);
    assert!(shared.script_exists("Child"));
    assert!(shared.can_resolve_script("Child"));
    assert!(!shared.can_resolve_script("Missing"));
    assert!(shared.type_exists("Int"));
    assert!(shared.has_state("States", "Active"));
    assert_eq!(
        shared.ancestor_states("States"),
        vec![("active".to_string(), false)]
    );
    assert_eq!(shared.is_global_function("Helpers", "Run"), Some(true));
    assert_eq!(
        shared.is_nodiscard_function("Helpers", "Run"),
        Some(false)
    );
    assert_eq!(
        shared.is_nodiscard_function("Helpers", "RegisterFoo"),
        Some(true)
    );
    assert!(shared.ancestry_fully_known("Child"));
}

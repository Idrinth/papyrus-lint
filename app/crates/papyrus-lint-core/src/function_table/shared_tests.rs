use super::super::test_support::write_script;
use super::*;
use papyrus_lints::ExternalSignatures;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::RwLock;

#[test]
fn shared_function_table_forwards_every_external_signature_lookup() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run() Global ; @deprecated\nEndFunction\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n",
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
    let table = RwLock::new(FunctionTable::new(root.path().to_path_buf()));
    let mut shared = SharedFunctionTable(&table);

    let params = shared
        .lookup("Helpers", "Run")
        .expect("function should resolve through the adapter");
    assert!(params.is_empty());
    assert!(shared.is_subtype("Child", "Helpers"));
    assert!(shared.has_property("Properties", "Name"));
    assert!(shared.has_field("Properties", "Age"));
    assert_eq!(shared.property_types("Properties"), vec!["String"]);
    let mut members: Vec<_> = shared
        .list_members("Properties")
        .into_iter()
        .map(|member| member.name().to_string())
        .collect();
    members.sort();
    assert_eq!(members, vec!["Name"]);
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
    assert!(shared.deprecated_function("Helpers", "Run").is_some());
    assert_eq!(shared.is_nodiscard_function("Helpers", "Run"), Some(false));
    assert_eq!(
        shared.function_has_side_effects("Helpers", "Run"),
        Some(false)
    );
    assert_eq!(
        shared.is_nodiscard_function("Helpers", "RegisterFoo"),
        Some(true)
    );
    assert!(shared.ancestry_fully_known("Child"));
    assert!(shared.function_access("Helpers", "Run").is_some());
    assert!(shared.property_access("Properties", "Name").is_some());

    // After the write-path fills the cache, a second pass is a read-lock
    // hit (no `ensure_loaded`). Holding the read guard across the calls
    // below deadlocks if any of them still takes the write lock.
    assert!(matches!(
        table
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .lookup_function_cached("Helpers", "Run"),
        super::super::CacheProbe::Hit(Some(_))
    ));
    let _held = table
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(shared.lookup("Helpers", "Run").is_some());
    assert!(shared.is_subtype("Child", "Helpers"));
    assert!(shared.has_property("Properties", "Name"));
    assert!(shared.has_field("Properties", "Age"));
    assert!(shared.has_state("States", "Active"));
    assert_eq!(
        shared.ancestor_states("States"),
        vec![("active".to_string(), false)]
    );
    assert_eq!(shared.is_global_function("Helpers", "Run"), Some(true));
    assert_eq!(shared.is_nodiscard_function("Helpers", "Run"), Some(false));
    assert!(shared.deprecated_function("Helpers", "Run").is_some());
    assert_eq!(
        shared.function_has_side_effects("Helpers", "Run"),
        Some(false)
    );
    assert!(shared.ancestry_fully_known("Child"));
    assert!(shared.function_access("Helpers", "Run").is_some());
    assert!(shared.property_access("Properties", "Name").is_some());
    assert_eq!(shared.property_types("Properties"), vec!["String"]);
    assert_eq!(shared.list_members("Properties").len(), 1);
}

#[test]
fn cached_negative_results_are_returned_without_a_write_lock() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Known", "ScriptName Known\n");
    let table = RwLock::new(FunctionTable::new(root.path().to_path_buf()));
    let mut shared = SharedFunctionTable(&table);

    assert_eq!(shared.lookup("Known", "Missing"), None);
    assert!(!shared.has_property("Known", "Missing"));
    assert!(!shared.has_field("Known", "Missing"));
    assert!(!shared.has_state("Known", "Missing"));
    assert_eq!(shared.is_global_function("Known", "Missing"), None);
    assert_eq!(shared.is_nodiscard_function("Known", "Missing"), None);
    assert_eq!(shared.deprecated_function("Known", "Missing"), None);
    assert_eq!(shared.function_has_side_effects("Known", "Missing"), None);
    assert!(shared.function_access("Known", "Missing").is_none());
    assert!(shared.property_access("Known", "Missing").is_none());

    // Every query above has a complete cached answer. Holding another read
    // guard proves that the adapter does not try to upgrade those hits to a
    // write lock.
    let _held = table
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(shared.lookup("Known", "Missing"), None);
    assert!(!shared.has_property("Known", "Missing"));
    assert!(!shared.has_field("Known", "Missing"));
    assert!(!shared.has_state("Known", "Missing"));
    assert_eq!(shared.is_global_function("Known", "Missing"), None);
    assert_eq!(shared.is_nodiscard_function("Known", "Missing"), None);
    assert_eq!(shared.deprecated_function("Known", "Missing"), None);
    assert_eq!(shared.function_has_side_effects("Known", "Missing"), None);
    assert!(shared.function_access("Known", "Missing").is_none());
    assert!(shared.property_access("Known", "Missing").is_none());
    assert!(shared.list_members("Known").is_empty());
    assert!(shared.property_types("Known").is_empty());
    assert!(shared.ancestor_states("Known").is_empty());
    assert!(shared.ancestry_fully_known("Known"));
}

#[test]
fn recovers_from_a_poisoned_lock_for_reads_and_cache_fills() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run()\nEndFunction\n",
    );
    let table = RwLock::new(FunctionTable::new(root.path().to_path_buf()));

    let poison_result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = table.write().expect("lock should start unpoisoned");
        panic!("poison the function table lock");
    }));
    assert!(poison_result.is_err());

    let mut shared = SharedFunctionTable(&table);
    assert!(shared.script_exists("Helpers"));
    assert!(shared.lookup("Helpers", "Run").is_some());
    assert!(shared
        .list_members("Helpers")
        .iter()
        .any(|member| member.name() == "Run"));
}

#[test]
fn script_exists_does_not_need_a_write_lock() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n");
    let table = RwLock::new(FunctionTable::new(root.path().to_path_buf()));
    // Hold a read lock across the lookup so a write lock would deadlock.
    let _held = table
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut shared = SharedFunctionTable(&table);
    assert!(shared.script_exists("Foo"));
    assert!(shared.type_exists("Int"));
    assert!(!shared.can_resolve_script("Missing"));
}

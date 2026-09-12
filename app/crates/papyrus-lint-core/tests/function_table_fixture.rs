//! Public-API integration coverage for `FunctionTable` using the fixture
//! scripts shared by the cross-script lint tests. These checks complement
//! the module's focused unit tests by exercising lazy file discovery,
//! parsing, and inheritance as one operation.

use std::path::PathBuf;

use papyrus_lint_core::function_table::{FunctionTable, Member};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn table() -> FunctionTable {
    let empty_root = tempfile::tempdir().expect("failed to create temp dir");
    FunctionTable::new_with_additional_roots(
        empty_root.path().to_path_buf(),
        vec![fixtures_dir().to_string_lossy().into_owned()],
    )
}

#[test]
fn inherited_function_lookup_preserves_the_declared_signature() {
    let mut table = table();

    let signature = table
        .lookup_function("typeb", "a")
        .expect("TypeB should inherit TypeA.A");

    assert_eq!(signature.name, "A");
    assert_eq!(
        signature.return_type.expect("A should return a value").name,
        "Form"
    );
    assert_eq!(signature.params.len(), 1);
    assert_eq!(signature.params[0].name, "aa");
    assert_eq!(signature.params[0].type_name.name, "TypeA");
    assert!(!signature.params[0].type_name.is_array);
    assert!(!signature.is_global);
    assert!(!signature.is_native);
    assert!(!signature.is_event);
    assert_eq!(signature.state, None);
}

#[test]
fn subtype_lookup_crosses_project_and_native_ancestors() {
    let mut table = table();

    assert!(table.is_subtype("TypeD", "TypeC"));
    assert!(table.is_subtype("typed", "armor"));
    assert!(table.is_subtype("TYPED", "FORM"));
    assert!(table.is_subtype("TypeD", "TypeD"));
    assert!(!table.is_subtype("TypeC", "TypeD"));
    assert!(!table.is_subtype("MissingType", "Form"));
}

#[test]
fn member_listing_includes_inherited_functions_and_properties() {
    let mut table = table();

    let members = table.list_members("TypeD");
    let mut function_names = members
        .iter()
        .filter_map(|member| match member {
            Member::Function(_) => Some(member.name().to_string()),
            Member::Property(_) => None,
        })
        .collect::<Vec<_>>();
    let mut property_names = members
        .iter()
        .filter_map(|member| match member {
            Member::Property(_) => Some(member.name().to_string()),
            Member::Function(_) => None,
        })
        .collect::<Vec<_>>();
    function_names.sort();
    property_names.sort();

    assert_eq!(function_names, ["GetAsA", "GetC", "GetD", "GetMe"]);
    assert_eq!(property_names, ["c", "d"]);
    assert!(table.has_property("typed", "C"));
    assert!(table.has_property("TypeD", "d"));
    assert!(!table.has_property("TypeD", "missing"));
}

#[test]
fn script_existence_covers_fixtures_native_globals_and_unknown_names() {
    let mut table = table();

    assert!(table.script_exists("TYPEA"));
    assert!(table.script_exists("game"));
    assert!(!table.script_exists("DefinitelyMissingScript"));
}

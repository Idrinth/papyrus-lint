use papyrus_parser::ast::TypeName;

use super::{ExternalSignatures, NoExternalSignatures, ParamInfo};

#[test]
fn no_external_signatures_leaves_member_metadata_unresolved() {
    let mut external = NoExternalSignatures;

    assert_eq!(external.lookup("Form", "GetName"), None);
    assert_eq!(external.function_access("Form", "GetName"), None);
    assert_eq!(external.property_access("Form", "Name"), None);
    assert_eq!(external.is_global_function("Game", "GetPlayer"), None);
    assert_eq!(external.is_nodiscard_function("Form", "GetName"), None);
    assert_eq!(external.deprecated_function("Form", "OldFunction"), None);
    assert_eq!(external.function_has_side_effects("Form", "Delete"), None);
}

#[test]
fn no_external_signatures_uses_conservative_existence_defaults() {
    let mut external = NoExternalSignatures;

    assert!(external.script_exists("UnknownScript"));
    assert!(external.type_exists("UnknownType"));
    assert!(external.has_state("UnknownScript", "UnknownState"));
    assert!(!external.can_resolve_script("UnknownScript"));
    assert!(!external.ancestry_fully_known("UnknownScript"));
}

#[test]
fn no_external_signatures_does_not_invent_relationships_or_members() {
    let mut external = NoExternalSignatures;

    assert!(!external.is_subtype("Armor", "Form"));
    assert!(!external.has_property("Form", "Name"));
    assert!(!external.has_field("Form", "Value"));
    assert!(external.ancestor_states("UnknownScript").is_empty());
    assert!(external.property_types("UnknownScript").is_empty());
}

#[test]
fn parameter_info_serializes_its_public_contract() {
    let parameter = ParamInfo {
        name: "akForm".to_string(),
        type_name: TypeName {
            name: "Form".to_string(),
            is_array: true,
        },
    };

    assert_eq!(
        serde_json::to_value(parameter).expect("ParamInfo serializes"),
        serde_json::json!({
            "name": "akForm",
            "type_name": {
                "name": "Form",
                "is_array": true
            }
        })
    );
}

use super::*;
use std::collections::HashMap;

#[test]
fn does_not_flag_anything_without_a_resolver() {
    let diagnostics = check("ScriptName A\n\nB Property Little Auto\n");

    assert!(diagnostics.is_empty());
}

struct FakeExternal {
    properties: HashMap<String, Vec<String>>,
}

impl FakeExternal {
    fn new(properties: &[(&str, &[&str])]) -> Self {
        FakeExternal {
            properties: properties
                .iter()
                .map(|(name, types)| {
                    (
                        name.to_ascii_lowercase(),
                        types.iter().map(|t| t.to_string()).collect(),
                    )
                })
                .collect(),
        }
    }
}

impl ExternalSignatures for FakeExternal {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        self.properties
            .get(&type_name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }
}

#[test]
fn flags_a_two_script_circular_dependency() {
    let mut external = FakeExternal::new(&[("B", &["A"])]);

    let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0]
        .message
        .contains("circular dependency: A -> B -> A"));
}

#[test]
fn flags_a_three_script_circular_dependency() {
    let mut external = FakeExternal::new(&[("B", &["C"]), ("C", &["A"])]);

    let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("circular dependency: A -> B -> C -> A"));
}

#[test]
fn does_not_flag_a_one_directional_dependency() {
    let mut external = FakeExternal::new(&[("B", &["C"])]);

    let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_direct_self_reference() {
    let mut external = FakeExternal::new(&[("A", &["A"])]);

    let diagnostics = check_with("ScriptName A\n\nA Property Next Auto\n", &mut external);

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_infinite_loop_on_a_cycle_that_never_reaches_the_origin() {
    let mut external = FakeExternal::new(&[("B", &["C"]), ("C", &["B"])]);

    let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_script_names_case_insensitively() {
    let mut external = FakeExternal::new(&[("b", &["a"])]);

    let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName A\n\nB Property (\n");
    assert!(diagnostics.is_empty());
}

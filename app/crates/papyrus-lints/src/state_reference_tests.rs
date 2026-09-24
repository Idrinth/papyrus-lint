use super::*;

#[derive(Default)]
struct States {
    expected_parent: &'static str,
    inherited: Vec<&'static str>,
    queries: Vec<(String, String)>,
}

impl ExternalSignatures for States {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.queries
            .push((type_name.to_string(), state_name.to_string()));
        assert_eq!(type_name, self.expected_parent);
        self.inherited
            .iter()
            .any(|name| name.eq_ignore_ascii_case(state_name))
    }
}

fn references(source: &str) -> StateReferences {
    let script = papyrus_parser::parse(source).expect("test script should parse");
    StateReferences::collect(&script)
}

#[test]
fn empty_and_local_states_are_never_missing() {
    let references = references(
        "ScriptName Example Extends BaseScript\n\
         State Waiting\n\
         EndState\n",
    );
    let mut external = States {
        expected_parent: "BaseScript",
        inherited: Vec::new(),
        queries: Vec::new(),
    };

    assert!(!references.is_missing("", &mut external));
    assert!(!references.is_missing("waiting", &mut external));
    assert!(!references.is_missing("WAITING", &mut external));
    assert!(external.queries.is_empty());
}

#[test]
fn script_without_a_parent_treats_unknown_state_as_missing() {
    let references = references("ScriptName Example\n");
    let mut external = States::default();

    assert!(references.is_missing("Unknown", &mut external));
    assert!(external.queries.is_empty());
}

#[test]
fn inherited_state_is_resolved_through_the_parent() {
    let references = references("ScriptName Example Extends BaseScript\n");
    let mut external = States {
        expected_parent: "BaseScript",
        inherited: vec!["Inherited"],
        queries: Vec::new(),
    };

    assert!(!references.is_missing("inherited", &mut external));
    assert!(references.is_missing("Unknown", &mut external));
    assert_eq!(
        external.queries,
        [
            ("BaseScript".to_string(), "inherited".to_string()),
            ("BaseScript".to_string(), "Unknown".to_string()),
        ]
    );
}

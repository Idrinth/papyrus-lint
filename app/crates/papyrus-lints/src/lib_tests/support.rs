//! Shared helpers for [`crate`]'s lint/repair unit tests.

use super::super::*;

pub(super) fn config_with(tweak: impl FnOnce(&mut Config)) -> Config {
    let mut config = Config::default();
    tweak(&mut config);
    config
}

pub(super) struct FakeExternalWithParentFunction;

impl external_signatures::ExternalSignatures for FakeExternalWithParentFunction {
    fn lookup(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(Vec::new())
        } else {
            None
        }
    }
}

pub(super) struct FakeExternalWithMissingScript;

impl external_signatures::ExternalSignatures for FakeExternalWithMissingScript {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("KnownScript")
    }
}

pub(super) struct FakeExternalWithCircularProperty;

impl external_signatures::ExternalSignatures for FakeExternalWithCircularProperty {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        if type_name.eq_ignore_ascii_case("B") {
            vec!["Example".to_string()]
        } else {
            Vec::new()
        }
    }
}

pub(super) struct FakeExternalWithNonGlobalFunction;

impl external_signatures::ExternalSignatures for FakeExternalWithNonGlobalFunction {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("NotGlobal")
        {
            Some(false)
        } else {
            None
        }
    }
}

pub(super) struct FakeExternalWithGlobalFunction;

impl external_signatures::ExternalSignatures for FakeExternalWithGlobalFunction {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("IsGlobal")
        {
            Some(true)
        } else {
            None
        }
    }
}

pub(super) struct FakeExternalWithRenamedParentParam;

impl external_signatures::ExternalSignatures for FakeExternalWithRenamedParentParam {
    fn lookup(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(vec![external_signatures::ParamInfo {
                name: "akTarget".to_string(),
                type_name: papyrus_parser::ast::TypeName {
                    name: "ObjectReference".to_string(),
                    is_array: false,
                },
            }])
        } else {
            None
        }
    }
}

pub(super) struct FakeExternalWithUnrelatedAncestry;

impl external_signatures::ExternalSignatures for FakeExternalWithUnrelatedAncestry {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Armor") || type_name.eq_ignore_ascii_case("Weapon")
    }
}

pub(super) struct FakeExternalWithUnusedImport;

impl external_signatures::ExternalSignatures for FakeExternalWithUnusedImport {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<external_signatures::ParamInfo>> {
        None
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Helpers")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_signatures::ExternalSignatures;

    #[test]
    fn config_with_applies_the_tweak_to_the_default_config() {
        let config = config_with(|config| config.rules.trailing_whitespace = false);

        assert!(!config.rules.trailing_whitespace);
        assert_eq!(
            config.rules.comma_spacing,
            Config::default().rules.comma_spacing
        );
    }

    #[test]
    fn parent_function_lookup_is_case_insensitive_and_rejects_other_functions() {
        let mut external = FakeExternalWithParentFunction;

        assert_eq!(external.lookup("parentscript", "dOtHiNg"), Some(Vec::new()));
        assert_eq!(external.lookup("OtherScript", "DoThing"), None);
        assert_eq!(external.lookup("ParentScript", "OtherFunction"), None);
    }

    #[test]
    fn missing_script_resolver_only_recognizes_the_known_script() {
        let mut external = FakeExternalWithMissingScript;

        assert_eq!(external.lookup("KnownScript", "DoThing"), None);
        assert!(external.script_exists("knownscript"));
        assert!(!external.script_exists("MissingScript"));
    }

    #[test]
    fn circular_property_resolver_only_returns_the_example_dependency_for_b() {
        let mut external = FakeExternalWithCircularProperty;

        assert_eq!(external.lookup("B", "DoThing"), None);
        assert_eq!(external.property_types("b"), vec!["Example"]);
        assert!(external.property_types("C").is_empty());
    }

    #[test]
    fn non_global_function_resolver_only_classifies_its_fixture_function() {
        let mut external = FakeExternalWithNonGlobalFunction;

        assert_eq!(external.lookup("MyScript", "NotGlobal"), None);
        assert_eq!(
            external.is_global_function("myscript", "nOtGlObAl"),
            Some(false)
        );
        assert_eq!(
            external.is_global_function("OtherScript", "NotGlobal"),
            None
        );
        assert_eq!(
            external.is_global_function("MyScript", "OtherFunction"),
            None
        );
    }

    #[test]
    fn global_function_resolver_only_classifies_its_fixture_function() {
        let mut external = FakeExternalWithGlobalFunction;

        assert_eq!(external.lookup("MyScript", "IsGlobal"), None);
        assert_eq!(
            external.is_global_function("myscript", "iSgLoBaL"),
            Some(true)
        );
        assert_eq!(external.is_global_function("OtherScript", "IsGlobal"), None);
        assert_eq!(
            external.is_global_function("MyScript", "OtherFunction"),
            None
        );
    }

    #[test]
    fn renamed_parent_parameter_lookup_returns_the_expected_signature() {
        let mut external = FakeExternalWithRenamedParentParam;

        let params = external.lookup("parentscript", "dOtHiNg").unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "akTarget");
        assert_eq!(params[0].type_name.name, "ObjectReference");
        assert!(!params[0].type_name.is_array);
        assert_eq!(external.lookup("OtherScript", "DoThing"), None);
        assert_eq!(external.lookup("ParentScript", "OtherFunction"), None);
    }

    #[test]
    fn unrelated_ancestry_resolver_only_knows_its_fixture_types() {
        let mut external = FakeExternalWithUnrelatedAncestry;

        assert_eq!(external.lookup("Armor", "DoThing"), None);
        assert!(external.is_subtype("Armor", "armor"));
        assert!(!external.is_subtype("Armor", "Weapon"));
        assert!(external.ancestry_fully_known("armor"));
        assert!(external.ancestry_fully_known("WEAPON"));
        assert!(!external.ancestry_fully_known("Form"));
    }

    #[test]
    fn unused_import_resolver_only_resolves_helpers() {
        let mut external = FakeExternalWithUnusedImport;

        assert_eq!(external.lookup("Helpers", "DoThing"), None);
        assert!(external.can_resolve_script("helpers"));
        assert!(!external.can_resolve_script("OtherScript"));
    }
}

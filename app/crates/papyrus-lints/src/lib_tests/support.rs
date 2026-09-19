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

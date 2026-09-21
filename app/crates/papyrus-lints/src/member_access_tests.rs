use papyrus_parser::ast::AccessLevel;

use super::*;
use crate::{ExternalSignatures, MemberAccess, ParamInfo};

#[derive(Default)]
struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn function_access(&mut self, type_name: &str, name: &str) -> Option<MemberAccess> {
        access(type_name, name)
    }

    fn property_access(&mut self, type_name: &str, name: &str) -> Option<MemberAccess> {
        access(type_name, name)
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
            || (sub_type.eq_ignore_ascii_case("Child") && super_type.eq_ignore_ascii_case("Base"))
    }
}

fn access(type_name: &str, name: &str) -> Option<MemberAccess> {
    let access_level = if name.eq_ignore_ascii_case("Secret") {
        AccessLevel::Private
    } else if name.eq_ignore_ascii_case("Inherited") {
        AccessLevel::Protected
    } else {
        AccessLevel::Public
    };
    type_name.eq_ignore_ascii_case("Base").then(|| MemberAccess {
        declaring_type: "Base".into(),
        access_level,
    })
}

fn lint(source: &str, config: &crate::Config) -> Vec<Diagnostic> {
    crate::lint_with_external_arguments(source, config, &mut FakeExternal)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == RULE)
        .collect()
}

#[test]
fn rejects_private_function_and_property_from_another_script() {
    let diagnostics = lint(
        "ScriptName Other\nBase Property Target Auto\nFunction Test()\n Target.Secret()\n Int value = Target.Secret\nEndFunction\n",
        &crate::Config::default(),
    );
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.message.starts_with("[error]")));
    assert_eq!(diagnostics[1].line, 5);
}

#[test]
fn permits_public_access_and_protected_access_from_an_inheritor() {
    let diagnostics = lint(
        "ScriptName Child Extends Base\nBase Property Target Auto\nFunction Test()\n Target.Inherited()\n Target.PublicMember()\nEndFunction\n",
        &crate::Config::default(),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn rejects_protected_access_from_an_unrelated_script() {
    let diagnostics = lint(
        "ScriptName Other\nBase Property Target Auto\nFunction Test()\n Target.Inherited()\nEndFunction\n",
        &crate::Config::default(),
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn disable_directives_and_config_are_honored() {
    let source = "ScriptName Other\nBase Property Target Auto\nFunction Test()\n Target.Secret() ; @disable member-access\nEndFunction\n";
    assert!(lint(source, &crate::Config::default()).is_empty());

    let mut config = crate::Config::default();
    config.rules.member_access = false;
    assert!(lint(source, &config).is_empty());
}

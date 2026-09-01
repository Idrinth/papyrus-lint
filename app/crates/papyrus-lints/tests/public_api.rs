//! Black-box tests for the crate-level lint and repair entry points.

use papyrus_lints::{
    argument_types::{ExternalSignatures, ParamInfo},
    lint, lint_with_external_arguments, repair, Config, Diagnostic,
};
use papyrus_parser::ast::TypeName;

#[test]
fn lint_reports_multiple_enabled_rules_through_the_public_api() {
    let source = "ScriptName Example  \n\nFunction Run(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
}

#[test]
fn repair_is_idempotent_and_clears_fixable_diagnostics() {
    let source = "ScriptName Example  \r\n\r\nFunction Run(Int left,Int right)\r\nEndFunction\r\n";
    let config = Config::default();

    let repaired = repair(source, &config);

    assert_eq!(
        repaired,
        "ScriptName Example\r\n\r\nFunction Run(Int Left, Int Right)\r\nEndFunction\r\n"
    );
    assert_eq!(repair(&repaired, &config), repaired);
    assert!(lint(&repaired, &config).iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "trailing-whitespace" | "comma-spacing" | "identifier-casing"
        )
    }));
}

#[test]
fn diagnostic_serialization_exposes_the_structured_output_contract() {
    let diagnostic = Diagnostic {
        line: 7,
        column: 11,
        message: "[warning] Example finding".to_string(),
        rule: "example-rule",
    };

    assert_eq!(
        serde_json::to_value(diagnostic).unwrap(),
        serde_json::json!({
            "line": 7,
            "column": 11,
            "message": "[warning] Example finding",
            "rule": "example-rule",
        })
    );
}

struct MissingScriptResolver;

impl ExternalSignatures for MissingScriptResolver {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("KnownScript")
    }
}

#[test]
fn external_resolver_is_used_by_the_public_lint_entry_point() {
    let source = "ScriptName Example\n\nFunction Run()\n    MissingScript.DoThing()\n    KnownScript.DoThing()\nEndFunction\n";

    let diagnostics =
        lint_with_external_arguments(source, &Config::default(), &mut MissingScriptResolver);
    let unresolved: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unresolved-script")
        .collect();

    assert_eq!(unresolved.len(), 1);
    assert_eq!((unresolved[0].line, unresolved[0].column), (4, 26));
    assert!(unresolved[0].message.contains("MissingScript"));
}

#[test]
fn public_lint_entry_point_uses_external_signatures_and_subtyping() {
    #[derive(Default)]
    struct ItemCountResolver {
        lookups: Vec<(String, String)>,
        subtype_checks: Vec<(String, String)>,
    }

    impl ExternalSignatures for ItemCountResolver {
        fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
            self.lookups
                .push((type_name.to_string(), function_name.to_string()));
            (type_name.eq_ignore_ascii_case("ObjectReference")
                && function_name.eq_ignore_ascii_case("GetItemCount"))
            .then(|| {
                vec![ParamInfo {
                    name: "akItem".to_string(),
                    type_name: TypeName {
                        name: "Form".to_string(),
                        is_array: false,
                    },
                }]
            })
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            self.subtype_checks
                .push((sub_type.to_string(), super_type.to_string()));
            sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
        }
    }

    let source = "ScriptName Example\n\nArmor Property MyArmor Auto\nWeapon Property MyWeapon Auto\n\nFunction CountItems(ObjectReference Container)\n    Container.GetItemCount(MyArmor)\n    Container.GetItemCount(MyWeapon)\nEndFunction\n";
    let mut resolver = ItemCountResolver::default();

    let diagnostics = lint_with_external_arguments(source, &Config::default(), &mut resolver);
    let argument_type_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "argument-types")
        .collect();

    assert_eq!(argument_type_diagnostics.len(), 1);
    assert_eq!(argument_type_diagnostics[0].line, 8);
    assert!(argument_type_diagnostics[0]
        .message
        .contains("expects Form"));
    assert!(argument_type_diagnostics[0].message.contains("got Weapon"));
    assert!(resolver
        .lookups
        .iter()
        .any(|(type_name, function)| type_name == "ObjectReference" && function == "GetItemCount"));
    assert!(resolver
        .subtype_checks
        .contains(&("Armor".to_string(), "Form".to_string())));
    assert!(resolver
        .subtype_checks
        .contains(&("Weapon".to_string(), "Form".to_string())));
}

#[test]
fn deserialized_formatting_config_drives_public_repair() {
    let config: Config = serde_yaml::from_str(
        "semicolon: true\nindentation: space\nindentation_width: 2\nrules:\n  identifier_casing: false\n",
    )
    .unwrap();
    let source = "Function run()\nIf ready\nDoThing()\nEndIf\nEndFunction\n";

    let repaired = repair(source, &config);

    assert_eq!(
        repaired,
        "Function run();\n  If ready;\n    DoThing();\n  EndIf;\nEndFunction;\n"
    );
    assert!(lint(&repaired, &config)
        .iter()
        .all(|diagnostic| { !matches!(diagnostic.rule, "semicolon" | "indentation") }));
}

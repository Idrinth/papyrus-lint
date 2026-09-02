//! Black-box tests for the crate-level lint and repair entry points.

use papyrus_lints::{
    argument_types::{ExternalSignatures, ParamInfo},
    lint, lint_with_external_arguments, repair, repair_filtered, Config, Diagnostic,
    FIXABLE_RULE_IDS, KNOWN_RULE_IDS,
};
use papyrus_parser::ast::TypeName;
use std::collections::HashSet;

#[test]
fn published_rule_id_lists_are_unique_and_fixable_rules_are_known() {
    let known: HashSet<_> = KNOWN_RULE_IDS.iter().copied().collect();
    let fixable: HashSet<_> = FIXABLE_RULE_IDS.iter().copied().collect();

    assert_eq!(known.len(), KNOWN_RULE_IDS.len(), "duplicate known rule id");
    assert_eq!(
        fixable.len(),
        FIXABLE_RULE_IDS.len(),
        "duplicate fixable rule id"
    );
    assert!(
        fixable.is_subset(&known),
        "every fixable rule must also be advertised as known"
    );
}

#[test]
fn every_published_fixable_rule_works_through_the_filtered_public_api() {
    let mut property_config = Config::default();
    property_config.rules.property_sorting = true;

    let default_config = Config::default();
    let cases = [
        (
            "identifier-casing",
            "ScriptName Example\n\nFunction Run(Int left)\nEndFunction\n",
            "ScriptName Example\n\nFunction Run(Int Left)\nEndFunction\n",
            &default_config,
        ),
        (
            "semicolon",
            "Int Value = 1;\n",
            "Int Value = 1\n",
            &default_config,
        ),
        (
            "indentation",
            "Function Run()\n  Call()\nEndFunction\n",
            "Function Run()\n\tCall()\nEndFunction\n",
            &default_config,
        ),
        (
            "property-sorting",
            "ScriptName Example\n\nInt Property Zulu Auto\nActor Property Alpha Auto\n",
            "ScriptName Example\nActor Property Alpha Auto\n\nInt Property Zulu Auto\n\n",
            &property_config,
        ),
        (
            "comma-spacing",
            "Call(1,2)\n",
            "Call(1, 2)\n",
            &default_config,
        ),
        (
            "chain-whitespace",
            "Value . Call()\n",
            "Value.Call()\n",
            &default_config,
        ),
        (
            "exclamation-spacing",
            "If !Ready\nEndIf\n",
            "If ! Ready\nEndIf\n",
            &default_config,
        ),
        (
            "operator-spacing",
            "If Left==Right\nEndIf\n",
            "If Left == Right\nEndIf\n",
            &default_config,
        ),
        (
            "type-casing",
            "ScriptName myScript\n",
            "ScriptName MyScript\n",
            &default_config,
        ),
        (
            "trailing-whitespace",
            "Call()  \n",
            "Call()\n",
            &default_config,
        ),
    ];

    let exercised: HashSet<_> = cases.iter().map(|(rule, ..)| *rule).collect();
    let published: HashSet<_> = FIXABLE_RULE_IDS.iter().copied().collect();
    assert_eq!(
        exercised, published,
        "add a filtered-repair case whenever the public fixable list changes"
    );

    for (rule, source, expected, config) in cases {
        assert_eq!(
            repair_filtered(source, config, Some(rule)),
            expected,
            "filtered repair failed for {rule}"
        );
    }
}

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

#[test]
fn rule_specific_disable_comment_only_suppresses_the_named_rule() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable comma-spacing   \nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 3 && diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 3 && diagnostic.rule == "trailing-whitespace"));
}

#[test]
fn bare_disable_comment_suppresses_all_findings_on_its_line_only() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable   \nFunction Other(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line != 3));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 4 && diagnostic.rule == "comma-spacing"));
}

#[test]
fn repair_applies_fixes_even_to_findings_hidden_by_disable_comments() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable comma-spacing   \nEndFunction\n";

    let repaired = repair(source, &Config::default());

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Run(Int Left, Int Right) ; @disable comma-spacing\nEndFunction\n"
    );
    assert_eq!(repair(&repaired, &Config::default()), repaired);
}

#[test]
fn public_diagnostics_have_valid_locations_and_severity_tags() {
    let source = "ScriptName Example  \n\nFunction Run(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(!diagnostics.is_empty());
    for diagnostic in diagnostics {
        assert!(diagnostic.line > 0, "{} had a zero line", diagnostic.rule);
        assert!(
            diagnostic.column > 0,
            "{} had a zero column",
            diagnostic.rule
        );
        assert!(
            diagnostic
                .message
                .starts_with(&format!("[{}]", diagnostic.level())),
            "{} did not have a recognized severity tag: {}",
            diagnostic.rule,
            diagnostic.message
        );
    }
}

#[test]
fn raw_source_rules_still_report_when_the_script_does_not_parse() {
    let source = "ScriptName Example\n\nFunction Broken(\n    Call(1,2)  \n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 4 && diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.line == 4 && diagnostic.rule == "trailing-whitespace" }));
}

#[test]
fn yaml_rule_switches_gate_both_public_lint_and_repair() {
    let config: Config = serde_yaml::from_str(
        "rules:\n  comma_spacing: false\n  trailing_whitespace: false\n  identifier_casing: false\n",
    )
    .unwrap();
    let source = "Function run(Int left,Int right)  \nEndFunction\n";

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "comma-spacing" | "trailing-whitespace" | "identifier-casing"
        )
    }));
    assert_eq!(repair(source, &config), source);
}

#[test]
fn public_repair_preserves_generated_fragment_wrapper_lines() {
    let source = ";BEGIN FRAGMENT CODE - generated  \nFunction Fragment_0(Int left,Int right)  \n;BEGIN CODE\nCall(left,right)  \n;END CODE\nEndFunction  \n;END FRAGMENT CODE  \n";

    let repaired = repair(source, &Config::default());

    assert_eq!(
        repaired,
        ";BEGIN FRAGMENT CODE - generated  \nFunction Fragment_0(Int left,Int right)  \n;BEGIN CODE\n\tCall(left, right)\n;END CODE\nEndFunction  \n;END FRAGMENT CODE  \n"
    );
    let diagnostics = lint(&repaired, &Config::default());
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.line != 4
            || !matches!(
                diagnostic.rule,
                "comma-spacing" | "trailing-whitespace" | "indentation"
            )
    }));
}

#[test]
fn disable_rule_ids_are_case_insensitive_and_accept_a_list() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable COMMA-SPACING, identifier-CASING\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.line != 3 || !matches!(diagnostic.rule, "comma-spacing" | "identifier-casing")
    }));
}

#[test]
fn public_repair_composes_all_fixable_rules_in_one_pass() {
    let mut config = Config::default();
    config.rules.property_sorting = true;
    let source = "ScriptName myScript  ;\n\nInt Property zulu Auto  ;\nActor Property alpha Auto  ;\n\nFunction run(Int left,Int right)  ;\n  If !Ready&&left==right  ;\n  alpha . MoveTo(None,left)  ;\n  EndIf  ;\nEndFunction  ;\n";

    let repaired = repair(source, &config);

    assert_eq!(
        repaired,
        "ScriptName MyScript\nActor Property Alpha Auto\n\nInt Property Zulu Auto\n\nFunction Run(Int Left, Int Right)\n\tIf ! Ready && Left == Right\n\t\tAlpha.MoveTo(None, Left)\n\tEndIf\nEndFunction\n"
    );
    assert_eq!(repair(&repaired, &config), repaired);
    assert!(lint(&repaired, &config).iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "trailing-whitespace"
                | "comma-spacing"
                | "semicolon"
                | "indentation"
                | "chain-whitespace"
                | "exclamation-spacing"
                | "operator-spacing"
                | "identifier-casing"
                | "type-casing"
                | "property-sorting"
        )
    }));
}

#[test]
fn disabling_all_fixable_rules_makes_public_repair_a_noop() {
    let mut config = Config::default();
    config.rules.trailing_whitespace = false;
    config.rules.comma_spacing = false;
    config.rules.semicolon = false;
    config.rules.indentation = false;
    config.rules.chain_whitespace = false;
    config.rules.exclamation_spacing = false;
    config.rules.operator_spacing = false;
    config.rules.identifier_casing = false;
    config.rules.type_casing = false;
    config.rules.property_sorting = false;
    let source = "ScriptName my_script  ;\n\nFunction run(Int left,Int right)  ;\n  If !ready&&left==right  ;\n  value . Call(left,right)  ;\n  EndIf  ;\nEndFunction  ;\n";

    assert_eq!(repair(source, &config), source);
}

#[test]
fn public_lint_reports_unknown_and_untriggered_disable_directives() {
    let source = "ScriptName Example\n\nCall() ; @disable mystery-rule, comma-spacing\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unused-disable")
        .collect();

    assert_eq!(unused.len(), 2);
    assert_eq!((unused[0].line, unused[0].column), (3, 19));
    assert!(unused[0].message.contains("mystery-rule"));
    assert!(unused[0].message.contains("unknown"));
    assert_eq!((unused[1].line, unused[1].column), (3, 33));
    assert!(unused[1].message.contains("comma-spacing"));
    assert!(unused[1].message.contains("does not produce"));
}

#[test]
fn public_lint_does_not_report_a_disable_that_suppresses_a_finding() {
    let source = "ScriptName Example\n\nCall(1,2) ; @disable comma-spacing\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.rule != "comma-spacing" && diagnostic.rule != "unused-disable"
    }));
}

#[test]
fn bare_disable_is_unused_only_when_its_line_has_no_findings() {
    let source = "ScriptName Example\n\nCall() ; @disable\nCall(1,2) ; @disable\n";
    let mut config = Config::default();
    config.rules.unused_disable = true;

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unused-disable")
        .collect();

    assert_eq!(unused.len(), 1);
    assert_eq!((unused[0].line, unused[0].column), (3, 10));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line != 4));
}

#[test]
fn unused_disable_rule_can_be_disabled_without_affecting_suppression() {
    let mut config = Config::default();
    config.rules.unused_disable = false;
    let source = "ScriptName Example\n\nCall() ; @disable mystery-rule\nCall(1,2) ; @disable comma-spacing\n";

    let diagnostics = lint(source, &config);

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| { diagnostic.line != 4 || diagnostic.rule != "comma-spacing" }));
}

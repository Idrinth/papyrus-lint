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
    let config: Config = serde_norway::from_str(
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
        ";BEGIN FRAGMENT CODE - generated  \nFunction Fragment_0(Int left,Int right)  \n;BEGIN CODE\nCall(left, right)\n;END CODE\nEndFunction  \n;END FRAGMENT CODE  \n"
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
    config.rules.slow_functions = false;
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

#[test]
fn opt_in_rule_is_disabled_by_default_through_the_public_api() {
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0)\n    EndIf\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "global-variable-setvalue"));
}

#[test]
fn yaml_can_enable_an_opt_in_rule_through_the_public_api() {
    let config: Config =
        serde_norway::from_str("rules:\n  global_variable_setvalue: true\n").unwrap();
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0)\n    Else\n        Toggle.SetValue(0.0)\n    EndIf\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "global-variable-setvalue")
        .collect();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (7, 1));
    assert!(diagnostics[0].message.contains("does not change the value"));
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (9, 1));
    assert!(diagnostics[1]
        .message
        .contains("may be an unnecessary write"));
}

#[test]
fn disable_comment_suppresses_an_opt_in_rule_on_only_its_own_line() {
    let mut config = Config::default();
    config.rules.global_variable_setvalue = true;
    let source = "ScriptName Example\n\nGlobalVariable Property Toggle Auto\n\nFunction Run()\n    If Toggle.GetValue() == 1.0\n        Toggle.SetValue(1.0) ; @disable global-variable-setvalue\n    Else\n        Toggle.SetValue(0.0)\n    EndIf\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "global-variable-setvalue")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

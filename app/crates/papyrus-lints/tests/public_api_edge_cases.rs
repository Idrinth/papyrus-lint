//! Edge-case coverage for the crate's black-box lint and repair API.

use papyrus_lints::{
    lint, repair, repair_filtered, restrict_to_line,
    tags::{tags_for, Importance, RULE_TAGS},
    Config, KNOWN_RULE_IDS,
};

#[test]
fn published_rule_tags_have_the_same_order_and_cardinality_as_known_rules() {
    let tagged_rules: Vec<_> = RULE_TAGS.iter().map(|tags| tags.rule).collect();

    assert_eq!(tagged_rules, KNOWN_RULE_IDS);
}

#[test]
fn tag_lookup_exposes_stable_metadata_for_multi_kind_rules() {
    let tags = tags_for("FORBIDDEN-FUNCTIONS").expect("known rule should have tags");

    assert_eq!(tags.rule, "forbidden-functions");
    assert_eq!(tags.kinds, ["performance", "correctness"]);
    assert_eq!(tags.importance, Importance::Medium);
    assert!(!tags.auto_fixable());
    assert!(tags_for("not-a-published-rule").is_none());
}

#[test]
fn empty_source_is_a_stable_noop() {
    let config = Config::default();

    assert!(lint("", &config).is_empty());
    assert_eq!(repair("", &config), "");
    assert_eq!(repair_filtered("", &config, Some("comma-spacing")), "");
}

#[test]
fn clean_source_is_not_rewritten() {
    let source = "ScriptName Example\r\n\r\nFunction Run(Int Left, Int Right)\r\n\tCall(Left, Right)\r\nEndFunction\r\n";
    let config = Config::default();

    assert_eq!(repair(source, &config), source);
}

#[test]
fn filtered_repair_preserves_unicode_and_unrelated_text() {
    let source = "String Message = \"héllo,world\"\n; λ,μ\nCall(1,2)\n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("comma-spacing")),
        "String Message = \"héllo,world\"\n; λ,μ\nCall(1, 2)\n"
    );
}

#[test]
fn line_restriction_preserves_mixed_line_endings_outside_the_target() {
    let original = "Call(1,2)\r\nCall(3,4)\nCall(5,6)\r\n";
    let repaired = repair_filtered(original, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(original, &repaired, 2),
        Some("Call(1,2)\r\nCall(3, 4)\nCall(5,6)\r\n".to_string())
    );
}

#[test]
fn line_restriction_can_apply_a_fix_to_the_first_or_last_line() {
    let original = "Call(1,2)\nCall(3,4)";
    let repaired = repair_filtered(original, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(original, &repaired, 1),
        Some("Call(1, 2)\nCall(3,4)".to_string())
    );
    assert_eq!(
        restrict_to_line(original, &repaired, 2),
        Some("Call(1,2)\nCall(3, 4)".to_string())
    );
}

#[test]
fn opt_in_rules_are_dispatched_by_the_public_lint_api() {
    let cases: &[(&str, &str, fn(&mut Config))] = &[
        (
            "property-sorting",
            "ScriptName Example\n\nInt Property Zulu Auto\nActor Property Alpha Auto\n",
            |config| config.rules.property_sorting = true,
        ),
        (
            "unchecked-form-parameter",
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
            |config| config.rules.unchecked_form_parameter = true,
        ),
        (
            "magic-numbers",
            "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
            |config| config.rules.magic_numbers = true,
        ),
        (
            "native-function-usage",
            "ScriptName MyNativeLib\n\nFunction DoSomethingNative() Global Native\n",
            |config| config.rules.native_function_usage = true,
        ),
        (
            "repeated-getvalue",
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    ElseIf gv.GetValue() == 2.0\n    EndIf\nEndFunction\n",
            |config| config.rules.repeated_getvalue = true,
        ),
        (
            "global-variable-setvalue",
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    Else\n        gv.SetValue(0.0)\n    EndIf\nEndFunction\n",
            |config| config.rules.global_variable_setvalue = true,
        ),
        (
            "unused-disable",
            "ScriptName Example\n\nFunction Test() ; @disable comma-spacing\nEndFunction\n",
            |config| config.rules.unused_disable = true,
        ),
    ];

    for (rule, source, enable) in cases {
        let mut config = Config::default();
        enable(&mut config);

        let diagnostics = lint(source, &config);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == *rule),
            "public lint API did not dispatch opted-in rule {rule}: {diagnostics:?}"
        );
    }
}

#[test]
fn script_name_collisions_are_dispatched_and_can_be_suppressed_publicly() {
    let source = "ScriptName Example\n\nInt Property example Auto ; @disable SCRIPT-NAME-COLLISION\nInt Example = 1\n";

    let diagnostics: Vec<_> = lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "script-name-collision")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 1));
    assert_eq!(diagnostics[0].level(), "error");
    assert!(diagnostics[0].message.contains("Variable 'Example'"));
}

#[test]
fn invariant_loop_conditions_are_dispatched_for_functions_in_states() {
    let source = "ScriptName Example\n\nState Waiting\n    Function Poll(Int remaining)\n        While remaining > 0\n            Debug.Trace(remaining)\n        EndWhile\n    EndFunction\nEndState\n";

    let diagnostics: Vec<_> = lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "invariant-loop-condition")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (5, 9));
    assert_eq!(diagnostics[0].level(), "warning");
    assert!(diagnostics[0].message.contains("'remaining'"));
}

#[test]
fn default_enabled_rules_can_be_disabled_through_deserialized_config() {
    let config: Config = serde_yaml::from_str(
        "rules:\n  script_name_collision: false\n  invariant_loop_condition: false\n",
    )
    .unwrap();
    let source = "ScriptName Example\n\nInt Property Example Auto\n\nFunction Run(Int remaining)\n    While remaining > 0\n        Debug.Trace(remaining)\n    EndWhile\nEndFunction\n";

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "script-name-collision" | "invariant-loop-condition"
        )
    }));
}

#[test]
fn every_filtered_fixer_respects_its_deserialized_rule_switch() {
    let cases = [
        (
            "identifier-casing",
            "ScriptName Example\n\nFunction Run(Int left)\nEndFunction\n",
        ),
        ("slow-functions", "Value.SetValueInt(3)\n"),
        ("semicolon", "Int Value = 1;\n"),
        ("indentation", "Function Run()\n  Call()\nEndFunction\n"),
        (
            "property-sorting",
            "ScriptName Example\n\nInt Property Zulu Auto\nActor Property Alpha Auto\n",
        ),
        ("comma-spacing", "Call(1,2)\n"),
        ("chain-whitespace", "Value . Call()\n"),
        ("exclamation-spacing", "If !Ready\nEndIf\n"),
        ("operator-spacing", "If Left==Right\nEndIf\n"),
        ("type-casing", "ScriptName myScript\n"),
        ("trailing-whitespace", "Call()  \n"),
    ];

    for (rule, source) in cases {
        let config_key = rule.replace('-', "_");
        let enabled_yaml = format!("rules:\n  {config_key}: true\n");
        let enabled: Config = serde_yaml::from_str(&enabled_yaml).unwrap();
        assert_ne!(
            repair_filtered(source, &enabled, Some(rule)),
            source,
            "fixture must exercise the {rule} fixer"
        );

        let disabled_yaml = format!("rules:\n  {config_key}: false\n");
        let disabled: Config = serde_yaml::from_str(&disabled_yaml).unwrap();
        assert_eq!(
            repair_filtered(source, &disabled, Some(rule)),
            source,
            "disabled {rule} fixer changed the source"
        );
    }
}

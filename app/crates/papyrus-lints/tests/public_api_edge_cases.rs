//! Edge-case coverage for the crate's black-box lint and repair API.

use papyrus_lints::{
    add_disable_comment,
    argument_types::{ExternalSignatures, ParamInfo},
    lint, repair, repair_filtered, repaired_line, restrict_to_line,
    tags::{tags_for, Importance, RULE_TAGS},
    Config, KNOWN_RULE_IDS,
};

fn rules(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

struct FunctionKindResolver;

impl ExternalSignatures for FunctionKindResolver {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if !type_name.eq_ignore_ascii_case("Library") {
            return None;
        }
        if function_name.eq_ignore_ascii_case("InstanceMethod") {
            Some(false)
        } else if function_name.eq_ignore_ascii_case("GlobalMethod") {
            Some(true)
        } else {
            None
        }
    }
}

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
fn repaired_line_returns_only_the_requested_fix_preview() {
    let source = "Call(1,2)\r\nCall(3,4)\r\n";

    assert_eq!(
        repaired_line(source, &Config::default(), "comma-spacing", 2),
        Some("Call(3, 4)\r".to_string())
    );
}

#[test]
fn repaired_line_rejects_unknown_rules_and_unchanged_lines() {
    let source = "Call(1,2)\nCall(3, 4)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), "not-a-rule", 1),
        None
    );
    assert_eq!(
        repaired_line(source, &Config::default(), "comma-spacing", 2),
        None
    );
}

#[test]
fn public_disable_comment_can_suppress_multiple_rules_on_one_line() {
    let source = "Call(1,2)  \nCall(3,4)  \n";
    let updated = add_disable_comment(source, 1, &rules(&["comma-spacing", "trailing-whitespace"]));
    let diagnostics = lint(&updated, &Config::default());

    assert_eq!(
        updated,
        "Call(1,2)   ; @disable comma-spacing, trailing-whitespace\nCall(3,4)  \n"
    );
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line != 1));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 2 && diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.line == 2 && diagnostic.rule == "trailing-whitespace"));
}

#[test]
fn public_disable_comment_merges_without_duplicating_rule_ids() {
    let source = "Call(1,2) ; note @disable COMMA-SPACING\r\n";

    assert_eq!(
        add_disable_comment(source, 1, &rules(&["comma-spacing", "operator-spacing"])),
        "Call(1,2) ; note @disable COMMA-SPACING, operator-spacing\r\n"
    );
}

#[test]
fn public_disable_comment_is_a_noop_without_a_valid_target_and_rules() {
    let source = "Call(1,2)\n";

    assert_eq!(add_disable_comment(source, 1, &[]), source);
    assert_eq!(
        add_disable_comment(source, 0, &rules(&["comma-spacing"])),
        source
    );
    assert_eq!(
        add_disable_comment(source, 3, &rules(&["comma-spacing"])),
        source
    );
}

#[test]
fn opt_in_rules_are_dispatched_by_the_public_lint_api() {
    type OptInCase = (&'static str, &'static str, fn(&mut Config));
    let cases: &[OptInCase] = &[
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
            "unchecked-array-element",
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
            |config| config.rules.unchecked_array_element = true,
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
            "default-property-value",
            "ScriptName Example\n\nInt Property Count Auto\n",
            |config| config.rules.default_property_value = true,
        ),
        (
            "unknown-actor-value",
            "ScriptName Example\n\nFunction Test(Actor target)\n    target.GetActorValue(\"Helth\")\nEndFunction\n",
            |config| config.rules.unknown_actor_value = true,
        ),
        (
            "unused-disable",
            "ScriptName Example\n\nFunction Test() ; @disable comma-spacing\nEndFunction\n",
            |config| config.rules.unused_disable = true,
        ),
        (
            "missing-doc-comment",
            "ScriptName Example\n\nFunction Test()\nEndFunction\n",
            |config| config.rules.missing_doc_comment = true,
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
fn default_property_value_is_opt_in_and_honors_line_disable_comments() {
    let source = "ScriptName Example\n\nBool Property Ready Auto ; @disable DEFAULT-PROPERTY-VALUE\nInt Property Count Auto\n";

    assert!(lint(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != "default-property-value"));

    let mut config = Config::default();
    config.rules.default_property_value = true;
    let diagnostics: Vec<_> = lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "default-property-value")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 1));
    assert!(diagnostics[0].message.contains("Count"));
    assert_eq!(diagnostics[0].level(), "warning");
}

#[test]
fn unknown_actor_value_is_opt_in_and_honors_file_disable_comments() {
    let source = "ScriptName Example ; @disable-file UNKNOWN-ACTOR-VALUE\n\nFunction Test(Actor target)\n    target.GetActorValue(\"Helth\")\nEndFunction\n";

    assert!(lint(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != "unknown-actor-value"));

    let mut config = Config::default();
    config.rules.unknown_actor_value = true;
    assert!(lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != "unknown-actor-value"));

    let source = source.replace(" ; @disable-file UNKNOWN-ACTOR-VALUE", "");
    let diagnostics: Vec<_> = lint(&source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "unknown-actor-value")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 12));
    assert_eq!(diagnostics[0].level(), "warning");
    assert!(diagnostics[0].message.contains("Helth"));
}

#[test]
fn missing_doc_comment_is_opt_in_and_honors_line_disable_comments() {
    // The ScriptName header carries its own doc comment, so only the
    // undocumented Function below is at stake here.
    let source =
        "ScriptName Example\n{doc}\n\nFunction Test() ; @disable MISSING-DOC-COMMENT\nEndFunction\n";

    assert!(lint(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != "missing-doc-comment"));

    let mut config = Config::default();
    config.rules.missing_doc_comment = true;
    assert!(lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != "missing-doc-comment"));

    let source = source.replace(" ; @disable MISSING-DOC-COMMENT", "");
    let diagnostics: Vec<_> = lint(&source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "missing-doc-comment")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 1));
    assert_eq!(diagnostics[0].level(), "warning");
    assert!(diagnostics[0].message.contains("Function `Test`"));
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
fn int_division_widening_is_reported_in_each_publicly_supported_context() {
    let source = "ScriptName Example\n\nFloat Function Ratio(Int numerator, Int denominator)\n    Float localRatio = numerator / denominator\n    localRatio = numerator / denominator\n    Consume(numerator / denominator)\n    Return numerator / denominator\nEndFunction\n\nFunction Consume(Float value)\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "int-division-to-float")
        .collect();

    assert_eq!(diagnostics.len(), 4);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        [4, 5, 6, 7]
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.level() == "warning" && diagnostic.message.contains("Int/Int division")
    }));
}

#[test]
fn int_division_widening_ignores_safe_float_operands_through_public_api() {
    let source = "ScriptName Example\n\nFloat Function Ratio(Int numerator, Int denominator)\n    Float first = numerator as Float / denominator\n    Float second = numerator / 2.0\n    Return first + second\nEndFunction\n";

    assert!(lint(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != "int-division-to-float"));
}

#[test]
fn disable_comment_suppresses_only_the_targeted_int_division_diagnostic() {
    let source = "ScriptName Example\n\nFloat Function Ratio(Int numerator, Int denominator)\n    Float ignored = numerator / denominator ; @disable INT-DIVISION-TO-FLOAT\n    Return numerator / denominator\nEndFunction\n";

    let diagnostics: Vec<_> = lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "int-division-to-float")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0]
        .message
        .contains("returned from Float function"));
}

#[test]
fn non_global_calls_use_external_function_metadata_through_the_public_api() {
    let source = "ScriptName Example\n\nFunction Run()\n    Library.InstanceMethod()\n    Library.GlobalMethod()\n    Library.UnknownMethod()\nEndFunction\n";

    let diagnostics: Vec<_> = lint_with_function_kinds(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "non-global-function-call")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "non-global-function-call");
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 27));
    assert_eq!(diagnostics[0].level(), "error");
    assert!(diagnostics[0].message.contains("Library.InstanceMethod()"));
}

#[test]
fn non_global_calls_honor_disable_comments_and_the_rule_switch() {
    let source = "ScriptName Example\n\nFunction Run()\n    Library.InstanceMethod() ; @disable NON-GLOBAL-FUNCTION-CALL\nEndFunction\n";
    assert!(lint_with_function_kinds(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != "non-global-function-call"));

    let disabled: Config =
        serde_yaml::from_str("rules:\n  non_global_function_call: false\n").unwrap();
    let source =
        "ScriptName Example\n\nFunction Run()\n    Library.InstanceMethod()\nEndFunction\n";
    assert!(lint_with_function_kinds(source, &disabled)
        .iter()
        .all(|diagnostic| diagnostic.rule != "non-global-function-call"));
}

fn lint_with_function_kinds(source: &str, config: &Config) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lints::lint_with_external_arguments(source, config, &mut FunctionKindResolver)
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
fn repeated_setoutfit_is_dispatched_and_honors_line_disable_comments() {
    let source = concat!(
        "ScriptName Example\n\n",
        "Function Dress(Actor Target, Outfit Clothes)\n",
        "    Target.SetOutfit(Clothes)\n",
        "    Target.SetOutfit(Clothes) ; @disable REPEATED-SETOUTFIT\n",
        "    Debug.Trace(\"still dressed\")\n",
        "    Target.SetOutfit(Clothes)\n",
        "EndFunction\n",
    );

    let diagnostics: Vec<_> = lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "repeated-setoutfit")
        .collect();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (7, 1));
    assert_eq!(diagnostics[0].level(), "warning");
    assert!(diagnostics[0]
        .message
        .contains("repeats the exact same outfit"));
}

#[test]
fn repeated_setoutfit_can_be_disabled_through_deserialized_config() {
    let config: Config = serde_yaml::from_str("rules:\n  repeated_setoutfit: false\n").unwrap();
    let source = concat!(
        "ScriptName Example\n\n",
        "Function Dress(Actor Target, Outfit Clothes)\n",
        "    Target.SetOutfit(Clothes)\n",
        "    Target.SetOutfit(Clothes)\n",
        "EndFunction\n",
    );

    assert!(lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != "repeated-setoutfit"));
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
        ("assignment-operator-spacing", "Left=Right\n"),
        ("type-casing", "ScriptName myScript\n"),
        ("trailing-whitespace", "Call()  \n"),
        (
            "global-variable-increment",
            "ScriptName Example\n\nFunction Test(GlobalVariable value)\n    value.SetValue(value.GetValue() + 1.0)\nEndFunction\n",
        ),
        (
            "unnecessary-function",
            "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n",
        ),
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

//! Black-box tests for the crate-level repair entry points: tag and line
//! filters, composition, formatting config, and idempotence.

use papyrus_lints::{
    lint, repair, repair_filtered, repair_filtered_by_tag, restrict_to_line, Config,
};

#[test]
fn tag_filtered_public_repair_applies_only_the_matching_kind() {
    let config = Config::default();
    let source =
        "ScriptName Example  \n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n";

    let performance_only = repair_filtered_by_tag(source, &config, Some("performance"));
    assert!(!performance_only.contains("GetValueInt"));
    assert!(performance_only.contains("Example  \n"));

    let style_only = repair_filtered_by_tag(source, &config, Some("style"));
    assert!(style_only.contains("GetValueInt"));
    assert!(style_only.contains("Example\n"));
    assert!(!style_only.contains("Example  \n"));

    assert_eq!(
        repair_filtered_by_tag(source, &config, None),
        repair(source, &config)
    );
}

#[test]
fn tag_filtered_public_repair_matches_case_insensitively_and_ignores_unknown_tags() {
    let config = Config::default();
    let source = "Call(1,2)  \r\n";

    assert_eq!(
        repair_filtered_by_tag(source, &config, Some("STYLE")),
        repair(source, &config)
    );
    assert_eq!(
        repair_filtered_by_tag(source, &config, Some("made-up-tag")),
        source
    );
}

#[test]
fn line_restricted_public_repair_changes_only_the_requested_line() {
    let source = "Call(1,2)  \r\nCall(3,4)  \r\nCall(5,6)  \r\n";
    let repaired = repair_filtered(source, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(source, &repaired, 2),
        Some("Call(1,2)  \r\nCall(3, 4)  \r\nCall(5,6)  \r\n".to_string())
    );
}

#[test]
fn line_restricted_public_repair_rejects_a_line_relocating_fix() {
    let mut config = Config::default();
    config.rules.property_sorting = true;
    let source = "ScriptName Example\n\nInt Property Zulu Auto\nActor Property Alpha Auto\n";
    let repaired = repair_filtered(source, &config, Some("property-sorting"));

    assert_ne!(source.lines().count(), repaired.lines().count());
    assert_eq!(restrict_to_line(source, &repaired, 3), None);
}

#[test]
fn line_restricted_public_repair_preserves_text_for_out_of_range_lines() {
    let source = "Call(1,2)\r\nCall(3,4)\r\n";
    let repaired = repair_filtered(source, &Config::default(), Some("comma-spacing"));

    assert_eq!(restrict_to_line(source, &repaired, 0), Some(source.into()));
    assert_eq!(restrict_to_line(source, &repaired, 4), Some(source.into()));
}

#[test]
fn line_restricted_public_repair_handles_a_final_line_without_a_newline() {
    let source = "Call(1,2)\nCall(3,4)";
    let repaired = repair_filtered(source, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(source, &repaired, 2),
        Some("Call(1,2)\nCall(3, 4)".to_string())
    );
}

#[test]
fn whitespace_repair_preserves_identifier_and_type_casing() {
    let source = "ScriptName exampleScript  \n\nFunction doThing(Int someValue)  \nEndFunction\n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("trailing-whitespace")),
        "ScriptName exampleScript\n\nFunction doThing(Int someValue)\nEndFunction\n"
    );
}

#[test]
fn repair_is_idempotent_and_clears_fixable_diagnostics() {
    let source = "ScriptName Example  \r\n\r\nFunction Run(Int left,Int right)\r\nEndFunction\r\n";
    let mut config = Config::default();
    // Isolate trailing-whitespace / comma-spacing / identifier-casing from
    // the default `line-endings` rewrite to LF.
    config.rules.line_endings = false;

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
fn unknown_filtered_repair_rule_is_a_noop() {
    let source = "Call(1,2)  \n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("not-a-rule")),
        source
    );
}

#[test]
fn a_disabled_rule_cannot_be_forced_through_filtered_repair() {
    let source = "Call(1,2)\n";
    let mut config = Config::default();
    config.rules.comma_spacing = false;

    assert_eq!(
        repair_filtered(source, &config, Some("comma-spacing")),
        source
    );
}

#[test]
fn deserialized_formatting_config_drives_public_repair() {
    let config: Config = serde_norway::from_str(
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
fn repair_applies_fixes_even_to_findings_hidden_by_disable_comments() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right) ; @disable comma-spacing   \nEndFunction\n";

    let repaired = repair(source, &Config::default());

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Run(Int Left, Int Right) ; @disable comma-spacing\nEndFunction\n"
    );
    assert_eq!(repair(&repaired, &Config::default()), repaired);
}

//! Edge-case coverage for the crate's black-box lint and repair API.

use papyrus_lints::{lint, repair, repair_filtered, restrict_to_line, Config};

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

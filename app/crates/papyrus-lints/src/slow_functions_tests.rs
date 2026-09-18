use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(tokens.as_deref())
}

fn check_with_rules(source: &str, rules: &'static [SlowFunctionRule]) -> Vec<Diagnostic> {
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check_with_rules(tokens.as_deref(), rules)
}

static GLOBAL_RULES: &[SlowFunctionRule] = &[SlowFunctionRule {
    object: "ExampleGlobal",
    function: "SlowCall",
    replacement: "FastCall",
    global: true,
}];

#[test]
fn compiled_rules_are_loaded_from_yaml() {
    assert_eq!(SLOW_FUNCTIONS.len(), 2);
    assert!(SLOW_FUNCTIONS
        .iter()
        .any(|r| r.object == "GlobalVariable" && r.function == "GetValueInt"));
}

#[test]
fn flags_qualified_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0]
        .message
        .contains("GlobalVariable.GetValueInt"));
    assert!(diagnostics[0].message.contains("GetValue() As Int"));
}

#[test]
fn flags_call_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.setvalueint(1)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("SetValueInt"));
}

#[test]
fn flags_code_before_an_inline_comment_but_ignores_calls_in_comment_text() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt() ; akGlobal.GetValueInt()\n    ; akGlobal.GetValueInt()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 14));
}

#[test]
fn global_rule_requires_its_literal_qualifier_case_insensitively() {
    let diagnostics = check_with_rules(
            "ExampleGlobal.SlowCall(1.0)\nexampleglobal.slowcall(1.0)\nakOther.SlowCall(1.0)\nSlowCall(1.0)\nGetGlobal().SlowCall(1.0)\n",
            GLOBAL_RULES,
        );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 2);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message.contains("ExampleGlobal.SlowCall")));
}

#[test]
fn does_not_flag_unrelated_calls() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"hi\")\n    self.DoOtherThing()\nEndFunction\n\nFunction DoOtherThing()\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_identifiers_that_are_not_calls() {
    let diagnostics = check("ScriptName Example\n\nInt GetValueInt = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction DoThing()\n    akGlobal.GetValueInt(\"unterminated\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn repairs_getter_with_the_complete_replacement_expression() {
    assert_eq!(
        repair("value = akGlobal.GetValueInt()\n"),
        "value = akGlobal.GetValue() As Int\n"
    );
}

#[test]
fn repairs_setter_and_preserves_its_argument_expression() {
    assert_eq!(
        repair("akGlobal.SetValueInt(GetAmount(1, 2) + 3)\n"),
        "akGlobal.SetValue(GetAmount(1, 2) + 3 As Float)\n"
    );
}

#[test]
fn repair_obeys_global_qualifier_rules() {
    let source = "ExampleGlobal.SlowCall(1.0)\nakOther.SlowCall(1.0)\n";

    assert_eq!(
        repair_with_rules(source, GLOBAL_RULES),
        "ExampleGlobal.FastCall(1.0)\nakOther.SlowCall(1.0)\n"
    );
}

#[test]
fn repairs_nested_slow_calls_and_handles_unicode_before_a_call() {
    let source = "text = \"é\"\nakGlobal.SetValueInt(akOther.GetValueInt())\n";

    assert_eq!(
        repair(source),
        "text = \"é\"\nakGlobal.SetValue(akOther.GetValue() As Int As Float)\n"
    );
}

#[test]
fn repairs_multiple_calls_on_the_same_line_from_right_to_left() {
    assert_eq!(
        repair("first = left.GetValueInt() + right.GetValueInt()\n"),
        "first = left.GetValue() As Int + right.GetValue() As Int\n"
    );
}

#[test]
fn repairs_a_call_after_unicode_on_the_same_line() {
    assert_eq!(
        repair("message = \"café\" + akGlobal.GetValueInt()\n"),
        "message = \"café\" + akGlobal.GetValue() As Int\n"
    );
}

#[test]
fn repair_does_not_change_calls_in_strings_or_comments() {
    let source = "text = \"GetValueInt()\" ; GetValueInt()\n; SetValueInt(1)\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_an_unbalanced_call_untouched() {
    let source = "akGlobal.SetValueInt(GetAmount(1, 2)\n";

    assert_eq!(repair(source), source);
}

#[test]
fn bare_replacement_preserves_empty_and_multiple_argument_lists() {
    assert_eq!(
        repair_with_rules(
            "ExampleGlobal.SlowCall()\nExampleGlobal.SlowCall(1, Other(2))\n",
            GLOBAL_RULES,
        ),
        "ExampleGlobal.FastCall()\nExampleGlobal.FastCall(1, Other(2))\n"
    );
}

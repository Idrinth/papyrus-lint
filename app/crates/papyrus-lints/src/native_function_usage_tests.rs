use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn compiled_rules_are_loaded_from_yaml() {
    assert!(!NATIVE_METHODS.is_empty());
    assert!(NATIVE_METHODS
        .iter()
        .any(|rule| rule.object == "Actor" && rule.function == "AddPerk"));
}

#[test]
fn does_not_flag_a_base_game_native_function() {
    let diagnostics = check(
        "ScriptName Actor\n\nFunction AddPerk(Perk akPerk, Bool abForceInform = true) Native\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_base_game_function_case_insensitively() {
    let diagnostics = check(
        "ScriptName actor\n\nFunction addperk(Perk akPerk, Bool abForceInform = true) Native\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_native_function_not_supplied_by_the_base_game() {
    let diagnostics =
        check("ScriptName MyNativeLib\n\nFunction DoSomethingNative(Int aiValue) Global Native\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0]
        .message
        .contains("MyNativeLib.DoSomethingNative"));
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_a_native_function_declared_inside_a_state() {
    let diagnostics = check(
            "ScriptName MyNativeLib\n\nState Busy\n    Function DoSomethingNative(Int aiValue) Global Native\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_a_function_with_a_body() {
    let diagnostics =
        check("ScriptName MyScript\n\nFunction DoThing()\n    Debug.Trace(\"hi\")\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_same_named_function_declared_on_an_unrelated_script() {
    let diagnostics = check("ScriptName MyActorHelper\n\nFunction AddPerk(Perk akPerk) Native\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("MyActorHelper.AddPerk"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction DoThing(\n");
    assert!(diagnostics.is_empty());
}

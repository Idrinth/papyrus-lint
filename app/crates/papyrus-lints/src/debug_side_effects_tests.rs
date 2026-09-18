use super::*;
use crate::config::Config;
use crate::lint;

fn script_with(body: &str) -> String {
    format!("ScriptName Example\n\nFunction Test(Actor akActor, Form item)\n{body}\nEndFunction\n")
}

#[test]
fn flags_removeitem_inside_debug_trace() {
    let source = script_with("    Debug.Trace(\"took \" + akActor.RemoveItem(item, 1))\n");
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("RemoveItem"));
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn flags_setvalue_inside_debug_notification() {
    let source = script_with("    Debug.Notification(akActor.SetActorValue(\"Health\", 0.0))\n");
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("SetActorValue"));
    assert!(diagnostics[0].message.contains("Debug.Notification"));
}

#[test]
fn flags_wait_inside_debug_messagebox() {
    let source = script_with("    Debug.MessageBox(\"waited \" + Utility.Wait(0.1))\n");
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Wait"));
}

#[test]
fn flags_same_script_function_that_writes_a_property() {
    let source = "ScriptName Example\n\nInt Property Count Auto\n\nInt Function Bump()\n    Count += 1\n    Return Count\nEndFunction\n\nFunction Test()\n    Debug.Trace(\"count=\" + Bump())\nEndFunction\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Bump"));
}

#[test]
fn flags_transitive_same_script_side_effect() {
    let source = "ScriptName Example\n\nInt Property Count Auto\n\nFunction Write()\n    Count = 1\nEndFunction\n\nInt Function Indirect()\n    Write()\n    Return Count\nEndFunction\n\nFunction Test()\n    Debug.Trace(Indirect())\nEndFunction\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Indirect"));
}

#[test]
fn does_not_flag_getter_inside_debug() {
    let source = script_with("    Debug.Trace(\"name=\" + akActor.GetName())\n");
    assert!(check(&source).is_empty());
}

#[test]
fn does_not_flag_same_script_pure_function() {
    let source = "ScriptName Example\n\nInt Function Twice(Int n)\n    Return n + n\nEndFunction\n\nFunction Test()\n    Debug.Trace(\"x=\" + Twice(3))\nEndFunction\n";
    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_debug_call_without_nested_call() {
    let source = script_with("    Debug.Trace(\"hello\")\n");
    assert!(check(&source).is_empty());
}

#[test]
fn does_not_flag_side_effect_outside_debug() {
    let source = script_with("    Int taken = akActor.RemoveItem(item, 1)\n    Debug.Trace(\"took \" + taken)\n");
    assert!(check(&source).is_empty());
}

#[test]
fn matches_debug_qualifier_case_insensitively() {
    let source = script_with("    debug.trace(akActor.RemoveItem(item, 1))\n");
    assert_eq!(check(&source).len(), 1);
}

#[test]
fn disable_comment_suppresses_the_finding() {
    let source = script_with(
        "    Debug.Trace(\"took \" + akActor.RemoveItem(item, 1)) ; @disable debug-side-effects\n",
    );
    let diagnostics = lint(&source, &Config::default());
    assert!(!diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn config_off_switch_disables_the_rule() {
    let source = script_with("    Debug.Trace(\"took \" + akActor.RemoveItem(item, 1))\n");
    let mut config = Config::default();
    config.rules.debug_side_effects = false;
    let diagnostics = lint(&source, &config);
    assert!(!diagnostics.iter().any(|d| d.rule == RULE));
}

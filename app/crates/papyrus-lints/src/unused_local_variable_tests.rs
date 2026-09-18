use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

#[test]
fn flags_local_variable_never_used_again() {
    let diagnostics = check("ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("declared but never used"));
    assert!(diagnostics[0].message.contains("'i'"));
}

#[test]
fn flags_write_only_local_variable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total = 1\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0]
        .message
        .contains("assigned a value but never used"));
    assert!(diagnostics[0].message.contains("'total'"));
}

#[test]
fn does_not_flag_variable_read_after_declaration() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    Debug.Trace(total)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_variable_read_via_return() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    Int total = 1\n    Return total\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_variable_used_via_compound_assignment() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    total += 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_variable_only_indexed_or_accessed_through() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] arr = new Int[3]\n    arr[0] = 5\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_variable_usage_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int total = 0\n    Debug.Trace(TOTAL)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_variable_declared_inside_if_block_never_used() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_variable_declared_in_if_and_used_after_it() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    EndIf\n    Debug.Trace(i)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_function_parameters() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(Int count)\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_script_properties() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue = 1 Auto\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int i = 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'i'"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_a_generated_local_declared_in_a_fragment_wrapper() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\n;NEXT FRAGMENT INDEX 0\nScriptname IDR__TIF__05000235 Extends TopicInfo Hidden\n\n;BEGIN FRAGMENT Fragment_0\nFunction Fragment_0(ObjectReference akSpeakerRef)\nActor akSpeaker = akSpeakerRef as Actor\n;BEGIN CODE\nPlayerRef.RemoveItem(Gold001, 5)\n;END CODE\nEndFunction\n;END FRAGMENT\n\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\nActor Property PlayerRef Auto\nMiscObject Property Gold001 Auto\n";

    assert!(check(source).is_empty());
}

#[test]
fn still_flags_an_unused_local_declared_inside_the_code_block() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\nScriptname Example Extends TopicInfo Hidden\nFunction Fragment_0(ObjectReference akSpeakerRef)\n;BEGIN CODE\nInt total = 1\n;END CODE\nEndFunction\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

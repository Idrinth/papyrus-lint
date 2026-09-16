use super::*;

fn tokens_of(source: &str) -> Vec<Token> {
    papyrus_parser::tokenize(source).expect("test source should tokenize")
}

#[test]
fn flags_script_header_with_no_doc_comment() {
    let diagnostics = check("ScriptName Example\n\nFunction Test()\nEndFunction\n");

    let script_finding = diagnostics
        .iter()
        .find(|d| d.message.contains("ScriptName Example"))
        .expect("script header should be flagged");
    assert_eq!(script_finding.line, 1);
    assert_eq!(script_finding.column, 1);
    assert_eq!(script_finding.rule, RULE);
    assert_eq!(script_finding.level(), "warning");
}

#[test]
fn does_not_flag_script_header_with_a_doc_comment() {
    let diagnostics = check(
            "ScriptName Example\n{Documentation for my cool script here!}\n\nFunction Test()\nEndFunction\n",
        );

    assert!(diagnostics
        .iter()
        .all(|d| !d.message.contains("ScriptName Example")));
}

#[test]
fn flags_property_with_no_doc_comment() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyProperty Auto\n");

    let property_finding = diagnostics
        .iter()
        .find(|d| d.message.contains("Property `MyProperty`"))
        .expect("property should be flagged");
    assert_eq!(property_finding.line, 3);
    assert_eq!(property_finding.column, 1);
}

#[test]
fn does_not_flag_property_with_a_doc_comment() {
    let diagnostics = check(
            "ScriptName Example\n{doc}\n\nInt Property MyProperty Auto\n{This property is fun, if you set it to 1, watch cool stuff happen!}\n",
        );

    assert!(diagnostics
        .iter()
        .all(|d| !d.message.contains("Property `MyProperty`")));
}

#[test]
fn flags_function_with_no_doc_comment() {
    let diagnostics = check("ScriptName Example\n{doc}\n\nFunction DoThing()\nEndFunction\n");

    let function_finding = diagnostics
        .iter()
        .find(|d| d.message.contains("Function `DoThing`"))
        .expect("function should be flagged");
    assert_eq!(function_finding.line, 4);
}

#[test]
fn does_not_flag_function_with_a_doc_comment() {
    let diagnostics = check(
            "ScriptName Example\n{doc}\n\nFunction DoThing()\n{Explains what DoThing does}\nEndFunction\n",
        );

    assert!(diagnostics
        .iter()
        .all(|d| !d.message.contains("Function `DoThing`")));
}

#[test]
fn flags_event_with_no_doc_comment() {
    let diagnostics = check("ScriptName Example\n{doc}\n\nEvent OnInit()\nEndEvent\n");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Event `OnInit`")));
}

#[test]
fn checks_functions_declared_inside_states_too() {
    let diagnostics = check(
            "ScriptName Example\n{doc}\n\nState Active\n    Function DoThing()\n    EndFunction\nEndState\n",
        );

    let function_finding = diagnostics
        .iter()
        .find(|d| d.message.contains("Function `DoThing`"))
        .expect("state function should be flagged");
    assert_eq!(function_finding.line, 5);
}

#[test]
fn a_multi_line_doc_comment_still_counts() {
    let diagnostics = check(
            "ScriptName Example\n{Documentation for my cool script here!\nI can even use more than one line...}\n\nFunction Test()\nEndFunction\n",
        );

    assert!(diagnostics
        .iter()
        .all(|d| !d.message.contains("ScriptName Example")));
}

#[test]
fn a_backslash_continued_function_header_checks_its_real_next_line() {
    let diagnostics = check(
            "ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\n{Explains what DoThing does}\nEndFunction\n",
        );

    assert!(diagnostics
        .iter()
        .all(|d| !d.message.contains("Function `DoThing`")));
}

#[test]
fn a_backslash_continued_function_header_without_a_doc_comment_is_still_flagged() {
    let diagnostics =
        check("ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\nEndFunction\n");

    let function_finding = diagnostics
        .iter()
        .find(|d| d.message.contains("Function `DoThing`"))
        .expect("function should be flagged");
    assert_eq!(function_finding.line, 4);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn a_declaration_on_the_last_line_with_no_following_line_is_flagged() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyProperty Auto");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Property `MyProperty`")));
}

#[test]
fn documentation_comment_returns_the_inner_text() {
    let source =
            "ScriptName Example\n{Documentation for my cool script here!}\n\nFunction Test()\nEndFunction\n";
    let tokens = tokens_of(source);

    assert_eq!(
        documentation_comment(source, &tokens, 1).as_deref(),
        Some("Documentation for my cool script here!")
    );
}

#[test]
fn documentation_comment_keeps_inner_newlines_of_a_multi_line_comment() {
    let source =
            "ScriptName Example\n{Documentation for my cool script here!\nI can even use more than one line...}\n";
    let tokens = tokens_of(source);

    assert_eq!(
        documentation_comment(source, &tokens, 1).as_deref(),
        Some("Documentation for my cool script here!\nI can even use more than one line...")
    );
}

#[test]
fn documentation_comment_follows_a_backslash_continued_header() {
    let source =
            "ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\n{Explains what DoThing does}\nEndFunction\n";
    let tokens = tokens_of(source);

    assert_eq!(
        documentation_comment(source, &tokens, 4).as_deref(),
        Some("Explains what DoThing does")
    );
}

#[test]
fn documentation_comment_is_none_when_the_declaration_has_no_comment() {
    let source = "ScriptName Example\n\nFunction DoThing()\nEndFunction\n";
    let tokens = tokens_of(source);

    assert_eq!(documentation_comment(source, &tokens, 1), None);
    assert_eq!(documentation_comment(source, &tokens, 3), None);
}

#[test]
fn documentation_comment_is_none_for_an_empty_brace_comment() {
    let source = "ScriptName Example\n{   }\n";
    let tokens = tokens_of(source);

    assert_eq!(documentation_comment(source, &tokens, 1), None);
}

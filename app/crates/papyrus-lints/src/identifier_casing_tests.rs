use super::*;

fn check(source: &str, style: IdentifierCasing) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        identifier_casing: style,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn repair(source: &str, style: IdentifierCasing) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        identifier_casing: style,
        ..Default::default()
    };
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

#[test]
fn flags_property_not_matching_pascal_case() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property myValue = 1 Auto\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Property"));
    assert!(diagnostics[0].message.contains("myValue"));
}

#[test]
fn does_not_flag_property_matching_pascal_case() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n",
        IdentifierCasing::PascalCase,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_script_level_variable_name() {
    let diagnostics = check(
        "ScriptName Example\n\nInt bad_name = 1\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("Variable"));
    assert!(diagnostics[0].message.contains("bad_name"));
}

#[test]
fn flags_function_name() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction do_thing()\nEndFunction\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Function"));
    assert!(diagnostics[0].message.contains("do_thing"));
}

#[test]
fn flags_event_name() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent on_init()\nEndEvent\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Event"));
    assert!(diagnostics[0].message.contains("on_init"));
}

#[test]
fn flags_parameter_on_the_function_line() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction DoThing(Int bad_name)\nEndFunction\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("Parameter"));
    assert!(diagnostics[0].message.contains("bad_name"));
}

#[test]
fn flags_local_variable_inside_a_function_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction DoThing()\n    Int bad_name = 1\nEndFunction\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("Variable"));
    assert!(diagnostics[0].message.contains("bad_name"));
}

#[test]
fn flags_local_variable_nested_in_if_and_while_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    If true\n        Int bad_one = 1\n    Else\n        Int bad_two = 2\n    EndIf\n    While true\n        Int bad_three = 3\n    EndWhile\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().any(|d| d.message.contains("bad_one")));
    assert!(diagnostics.iter().any(|d| d.message.contains("bad_two")));
    assert!(diagnostics.iter().any(|d| d.message.contains("bad_three")));
}

#[test]
fn flags_state_name() {
    let diagnostics = check(
        "ScriptName Example\n\nState bad_state\nEndState\n",
        IdentifierCasing::PascalCase,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("State"));
    assert!(diagnostics[0].message.contains("bad_state"));
}

#[test]
fn flags_function_declared_inside_a_state() {
    let diagnostics = check(
        "ScriptName Example\n\nState Idle\n    Function do_thing()\n    EndFunction\nEndState\n",
        IdentifierCasing::PascalCase,
    );

    assert!(diagnostics.iter().any(|d| d.message.contains("do_thing")));
}

#[test]
fn never_flags_script_name() {
    let diagnostics = check("ScriptName not_pascal_case\n", IdentifierCasing::PascalCase);

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_against_the_configured_style() {
    let source = "ScriptName Example\n\nInt Property my_value = 1 Auto\n";

    assert!(check(source, IdentifierCasing::SnakeCase).is_empty());
    assert!(!check(source, IdentifierCasing::PascalCase).is_empty());
}

#[test]
fn checks_camel_and_constant_case_styles() {
    let camel_case = "ScriptName Example\n\nInt Property myValue Auto\n";
    let constant_case = "ScriptName Example\n\nInt Property MY_VALUE Auto\n";

    assert!(check(camel_case, IdentifierCasing::CamelCase).is_empty());
    assert_eq!(
        check(camel_case, IdentifierCasing::ConstantCase)[0].message,
        "[warning] Property 'myValue' does not match the configured CONSTANT_CASE casing style"
    );
    assert!(check(constant_case, IdentifierCasing::ConstantCase).is_empty());
    assert_eq!(
        check(constant_case, IdentifierCasing::CamelCase)[0].message,
        "[warning] Property 'MY_VALUE' does not match the configured camelCase casing style"
    );
}

#[test]
fn flags_locals_nested_in_else_if_branches() {
    let source = "ScriptName Example\n\nFunction DoThing()\n    If false\n    ElseIf true\n        Int bad_name = 1\n    EndIf\nEndFunction\n";

    let diagnostics = check(source, IdentifierCasing::PascalCase);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("bad_name"));
}

#[test]
fn ignores_declarations_inside_a_fragment_wrapper() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname IDR__TIF__05000235 Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function fragment_0(ObjectReference akSpeakerRef)
Actor bad_local = akSpeakerRef as Actor
;BEGIN CODE
Int GoodStyle = 1
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";

    let diagnostics = check(source, IdentifierCasing::PascalCase);

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property bad_name = \"unterminated\n",
        IdentifierCasing::PascalCase,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn repair_renames_declarations_and_references() {
    let source = "ScriptName Example\n\nInt Property MaxCount Auto\n\nFunction AddValue(Int ItemCount)\n    Int NewTotal = MaxCount + ItemCount\n    MaxCount = NewTotal\nEndFunction\n";

    assert_eq!(
            repair(source, IdentifierCasing::CamelCase),
            "ScriptName Example\n\nInt Property maxCount Auto\n\nFunction addValue(Int itemCount)\n    Int newTotal = maxCount + itemCount\n    maxCount = newTotal\nEndFunction\n"
        );
}

#[test]
fn repair_matches_references_case_insensitively() {
    let source = "ScriptName Example\n\nInt Property MAXCOUNT Auto\n\nFunction Update()\n    maxcount = MaxCount + MAXCOUNT\nEndFunction\n";

    assert_eq!(
            repair(source, IdentifierCasing::CamelCase),
            "ScriptName Example\n\nInt Property maxcount Auto\n\nFunction update()\n    maxcount = maxcount + maxcount\nEndFunction\n"
        );
}

#[test]
fn repair_renames_state_members_and_nested_locals() {
    let source = "ScriptName Example\n\nState waitingState\n    Function handleEvent(Int itemCount)\n        If true\n            Int firstValue = itemCount\n        Else\n            While true\n                Int secondValue = firstValue\n            EndWhile\n        EndIf\n    EndFunction\nEndState\n";

    assert_eq!(
            repair(source, IdentifierCasing::PascalCase),
            "ScriptName Example\n\nState WaitingState\n    Function HandleEvent(Int ItemCount)\n        If true\n            Int FirstValue = ItemCount\n        Else\n            While true\n                Int SecondValue = FirstValue\n            EndWhile\n        EndIf\n    EndFunction\nEndState\n"
        );
}

#[test]
fn repair_handles_crlf_line_endings() {
    let source = "ScriptName Example\r\n\r\nInt Property MaxCount Auto\r\nFunction ReadValue()\r\n    Return MaxCount\r\nEndFunction\r\n";

    assert_eq!(
            repair(source, IdentifierCasing::CamelCase),
            "ScriptName Example\r\n\r\nInt Property maxCount Auto\r\nFunction readValue()\r\n    Return maxCount\r\nEndFunction\r\n"
        );
}

#[test]
fn repair_returns_already_conforming_source_unchanged() {
    let source = "ScriptName Example\n\nInt Property maxCount Auto\n";

    assert_eq!(repair(source, IdentifierCasing::CamelCase), source);
}

#[test]
fn repair_never_adds_removes_or_moves_underscores() {
    let source =
        "ScriptName Example\n\nFunction DFO_VampireFeed()\n    DFO_VampireFeed()\nEndFunction\n";

    assert_eq!(repair(source, IdentifierCasing::CamelCase), source);
    assert_eq!(
        repair(
            "ScriptName Example\n\nFunction HTTPResponseCode()\nEndFunction\n",
            IdentifierCasing::SnakeCase,
        ),
        "ScriptName Example\n\nFunction HTTPResponseCode()\nEndFunction\n"
    );
}

#[test]
fn repair_supports_every_casing_style() {
    assert_eq!(
        convert_name("HTTP_responseCode", IdentifierCasing::CamelCase),
        "httpResponseCode"
    );
    assert_eq!(
        convert_name("HTTP_responseCode", IdentifierCasing::PascalCase),
        "HttpResponseCode"
    );
    assert_eq!(
        convert_name("HTTPResponseCode", IdentifierCasing::SnakeCase),
        "http_response_code"
    );
    assert_eq!(
        convert_name("HTTPResponseCode", IdentifierCasing::ConstantCase),
        "HTTP_RESPONSE_CODE"
    );
}

#[test]
fn repair_leaves_script_name_comments_strings_and_fragment_wrapper_untouched() {
    let source = ";BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\nScriptName bad_name\nFunction generated_name()\n;BEGIN CODE\nInt userValue = 1 ; userValue\nString textValue = \"userValue\"\nuserValue = 2\n;END CODE\nEndFunction\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";
    let repaired = repair(source, IdentifierCasing::PascalCase);

    assert!(repaired.contains("ScriptName bad_name\nFunction generated_name()"));
    assert!(repaired.contains("Int UserValue = 1 ; userValue"));
    assert!(repaired.contains("String TextValue = \"userValue\""));
    assert!(repaired.contains("UserValue = 2"));
}

#[test]
fn repair_returns_unparseable_source_unchanged() {
    let source = "ScriptName Example\nFunction bad_name(\n";
    assert_eq!(repair(source, IdentifierCasing::PascalCase), source);
}

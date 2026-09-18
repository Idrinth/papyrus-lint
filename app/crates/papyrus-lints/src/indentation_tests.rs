use super::*;

fn check(source: &str, indentation: Indentation) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = match indentation {
        Indentation::Tabs => crate::config::Config {
            indentation: crate::config::Indentation::Tab,
            ..Default::default()
        },
        Indentation::Spaces(width) => crate::config::Config {
            indentation: crate::config::Indentation::Space,
            indentation_width: width,
            ..Default::default()
        },
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::argument_types::NoExternalSignatures,
    )
}

fn repair(source: &str, indentation: Indentation) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = match indentation {
        Indentation::Tabs => crate::config::Config {
            indentation: crate::config::Indentation::Tab,
            ..Default::default()
        },
        Indentation::Spaces(width) => crate::config::Config {
            indentation: crate::config::Indentation::Space,
            indentation_width: width,
            ..Default::default()
        },
    };
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

const SOURCE: &str = "ScriptName Example\nFunction Run()\nIf ready\nDoThing()\nElseIf waiting\nWait()\nElse\nStop()\nEndIf\nEndFunction\n";

#[test]
fn repairs_nested_blocks_with_spaces() {
    assert_eq!(
            repair(SOURCE, Indentation::Spaces(2)),
            "ScriptName Example\nFunction Run()\n  If ready\n    DoThing()\n  ElseIf waiting\n    Wait()\n  Else\n    Stop()\n  EndIf\nEndFunction\n"
        );
}

#[test]
fn repairs_nested_blocks_with_tabs() {
    let repaired = repair(SOURCE, Indentation::Tabs);
    assert!(repaired.contains("\tIf ready\n\t\tDoThing()\n"));
}

#[test]
fn preserves_crlf_blank_lines_and_final_line_ending() {
    let source = "Function Run()\r\n  \r\n    Return\r\nEndFunction";
    assert_eq!(
        repair(source, Indentation::Spaces(4)),
        "Function Run()\r\n\r\n    Return\r\nEndFunction"
    );
}

#[test]
fn native_functions_and_auto_properties_do_not_open_blocks() {
    let source = "Int Function GetValue() Native\nInt Property Value Auto\nInt x\n";
    assert_eq!(repair(source, Indentation::Spaces(2)), source);
}

#[test]
fn repair_is_idempotent() {
    let repaired = repair(SOURCE, Indentation::Spaces(3));
    assert_eq!(repair(&repaired, Indentation::Spaces(3)), repaired);
}

#[test]
fn malformed_source_is_left_unchanged() {
    let source = "Function Run()\n  @invalid\nEndFunction\n";
    assert_eq!(repair(source, Indentation::Tabs), source);
}

#[test]
fn flags_lines_indented_with_the_wrong_unit() {
    let source = "Function Run()\n  If ready\nDoThing()\nEndIf\nEndFunction\n";
    let diagnostics = check(source, Indentation::Tabs);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(diagnostics[0].line, 2);
    assert_eq!(diagnostics[0].column, 1);
    assert_eq!(
        diagnostics[0].message,
        "[warning] Line should be indented with 1 tab"
    );
    assert_eq!(diagnostics[1].line, 3);
    assert_eq!(
        diagnostics[1].message,
        "[warning] Line should be indented with 2 tabs"
    );
    assert_eq!(diagnostics[2].line, 4);
    assert_eq!(
        diagnostics[2].message,
        "[warning] Line should be indented with 1 tab"
    );
}

#[test]
fn flags_lines_indented_with_the_wrong_width() {
    let source = "Function Run()\n If ready\n  DoThing()\n EndIf\nEndFunction\n";
    let diagnostics = check(source, Indentation::Spaces(2));

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics[0].message,
        "[warning] Line should be indented with 2 spaces"
    );
    assert_eq!(
        diagnostics[1].message,
        "[warning] Line should be indented with 4 spaces"
    );
    assert_eq!(
        diagnostics[2].message,
        "[warning] Line should be indented with 2 spaces"
    );
}

#[test]
fn ignores_correctly_indented_source() {
    let repaired = repair(SOURCE, Indentation::Spaces(2));
    assert!(check(&repaired, Indentation::Spaces(2)).is_empty());
}

#[test]
fn ignores_blank_lines() {
    let source = "Function Run()\n\n   \nEndFunction\n";
    assert!(check(source, Indentation::Tabs).is_empty());
}

#[test]
fn ignores_malformed_source() {
    let source = "Function Run()\n  @invalid\nEndFunction\n";
    assert!(check(source, Indentation::Tabs).is_empty());
}

#[test]
fn checking_repaired_output_finds_nothing() {
    for indentation in [Indentation::Tabs, Indentation::Spaces(3)] {
        let repaired = repair(SOURCE, indentation);
        assert!(check(&repaired, indentation).is_empty());
    }
}

#[test]
fn fragment_code_wrapper_is_never_reindented() {
    // The function signature, the local variable declaration, `EndFunction`,
    // and every wrapper/marker comment must stay exactly as CreationKit wrote
    // them; only the actual code between `;BEGIN CODE`/`;END CODE` may be
    // reindented, and relative to that marker's own depth (matching how
    // CreationKit itself writes fragment code, flush with the marker
    // rather than nested inside the wrapper function around it) rather
    // than the file-wide depth of that wrapper function.
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    assert!(check(source, Indentation::Tabs).is_empty());
    assert_eq!(repair(source, Indentation::Tabs), source);
}

#[test]
fn fragment_code_is_indented_relative_to_its_begin_code_marker() {
    // A nested `If`/`EndIf` inside the code section should still gain
    // its own extra level, just measured from the marker rather than
    // from the top of the file.
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
If akSpeaker
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source, Indentation::Tabs);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);

    assert_eq!(
        repair(source, Indentation::Tabs),
        "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
If akSpeaker
\takSpeaker.RemoveItem(x, 1, false, PlayerRef)
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
    );
}

#[test]
fn real_creation_kit_fragment_is_left_unchanged() {
    // A real CreationKit-authored fragment (fixture shared with the
    // trailing-whitespace tests) already writes its code flush with the
    // `;BEGIN CODE` marker's own depth; the indentation fixer must not
    // treat the wrapper function's nesting as extra depth on top of that.
    let source = include_str!("../tests/fixtures/IDR__TIF__050002AB.psc");
    assert!(check(source, Indentation::Tabs).is_empty());
    assert_eq!(repair(source, Indentation::Tabs), source);
}

#[test]
fn deserializes_frontend_configuration() {
    assert_eq!(
        serde_json::from_str::<Indentation>(r#""Tabs""#).unwrap(),
        Indentation::Tabs
    );
    assert_eq!(
        serde_json::from_str::<Indentation>(r#"{"Spaces":4}"#).unwrap(),
        Indentation::Spaces(4)
    );
}

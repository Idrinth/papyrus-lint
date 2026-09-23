//! Error-path coverage for truncated blocks and Fallout 4-only declarations.

use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse, parse_with_mode, PapyrusError};

fn parse_error(source: &str) -> String {
    let PapyrusError::Parse(error) = parse(source).expect_err("source should not parse") else {
        panic!("expected a parser error for {source:?}");
    };
    error.message
}

fn fallout4_parse_error(source: &str) -> String {
    let PapyrusError::Parse(error) =
        parse_with_mode(source, GameEdition::Fallout4).expect_err("source should not parse")
    else {
        panic!("expected a parser error for {source:?}");
    };
    error.message
}

#[test]
fn reports_each_truncated_declaration_block_at_end_of_file() {
    for (source, expected) in [
        (
            "ScriptName Broken\nState Active\n",
            "expected EndState, found end of file",
        ),
        (
            "ScriptName Broken\nFunction Run()\n",
            "expected keyword EndFunction, found Eof",
        ),
        (
            "ScriptName Broken\nEvent OnInit()\n",
            "expected keyword EndEvent, found Eof",
        ),
    ] {
        assert_eq!(parse_error(source), expected);
    }
}

#[test]
fn reports_truncated_fallout4_structs_and_groups() {
    for (source, expected) in [
        (
            "ScriptName Broken\nStruct Position\nFloat X\n",
            "expected EndStruct, found end of file",
        ),
        (
            "ScriptName Broken\nGroup Settings\nInt Property Count Auto\n",
            "expected EndGroup, found end of file",
        ),
    ] {
        assert_eq!(fallout4_parse_error(source), expected);
    }
}

#[test]
fn rejects_invalid_members_inside_fallout4_declaration_blocks() {
    for (source, expected) in [
        (
            "ScriptName Broken\nStruct Position\nFunction Run() Native\nEndStruct\n",
            "expected identifier, found Keyword(Function)",
        ),
        (
            "ScriptName Broken\nGroup Settings\nInt value\nEndGroup\n",
            "expected something else impossible",
        ),
    ] {
        assert_eq!(fallout4_parse_error(source), expected);
    }
}

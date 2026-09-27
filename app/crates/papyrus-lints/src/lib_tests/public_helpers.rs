//! Tests for the small public helper entry points forwarded by [`crate`].

use super::super::*;

#[test]
fn check_argument_types_parses_source_before_checking_it() {
    let source = concat!(
        "ScriptName Example\n\n",
        "Function SetCount(Int count)\n",
        "EndFunction\n\n",
        "Function Test()\n",
        "    SetCount(1.5)\n",
        "EndFunction\n",
    );

    let diagnostics = check_argument_types(source, &mut external_signatures::NoExternalSignatures);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, argument_types::RULE);
    assert_eq!(diagnostics[0].line, 7);
    assert!(diagnostics[0].message.contains("expects Int"));
    assert!(diagnostics[0].message.contains("got Float"));
}

#[test]
fn check_argument_types_returns_no_diagnostics_for_unparseable_source() {
    let diagnostics = check_argument_types(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        &mut external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn documentation_comment_returns_the_comment_after_the_declaration() {
    let source = "ScriptName Example\n{Documents Example}\n";
    let tokens = papyrus_parser::tokenize(source).expect("test source should tokenize");

    assert_eq!(
        documentation_comment(source, &tokens, 1).as_deref(),
        Some("Documents Example")
    );
}

#[test]
fn literal_goto_state_targets_returns_lowercase_self_targets() {
    let source = concat!(
        "ScriptName Example\n\n",
        "Function Test(Example other, String stateName)\n",
        "    GoToState(\"Active\")\n",
        "    self.GoToState(\"INACTIVE\")\n",
        "    other.GoToState(\"Ignored\")\n",
        "    GoToState(stateName)\n",
        "EndFunction\n",
    );
    let script = papyrus_parser::parse(source).expect("test source should parse");

    let targets = literal_goto_state_targets(&script);

    assert_eq!(targets.len(), 2);
    assert!(targets.contains("active"));
    assert!(targets.contains("inactive"));
}

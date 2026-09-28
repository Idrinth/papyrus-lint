use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn flags_method_call_on_chained_call_result() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    ObjectReference a\n    a.GetLinkedRef().Disable()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains(".Disable"));
    assert!(diagnostics[0].message.contains("chained"));
}

#[test]
fn flags_property_chain_member_access() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    akRef.LinkedRef.Disable()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains(".Disable"));
}

#[test]
fn flags_cast_receiver_member_access() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).Kill()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains(".Kill"));
}

#[test]
fn flags_chain_rooted_at_script_property() {
    let diagnostics = check(
        "ScriptName Example\n\nObjectReference Property Target Auto\n\nFunction Test()\n    Target.GetLinkedRef().Disable()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_chain_rooted_at_self() {
    let diagnostics = check(
        "ScriptName Example extends ObjectReference\n\nFunction Test()\n    Self.GetLinkedRef().Disable()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_bare_identifier_receiver() {
    // Owned by none-form-usage; this sibling must not duplicate it.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_static_script_qualifier_chains() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Game.GetPlayer().AddItem(None, 1)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_array_index_receiver() {
    // Owned by unchecked-array-element.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_duplicate_on_longer_chain_inner_identifier_step() {
    // a.GetLinkedRef() has Identifier receiver — skipped; only the outer
    // .Disable on the call result is flagged.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().Disable()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains(".Disable"));
    assert!(!diagnostics[0].message.contains(".GetLinkedRef"));
}

#[test]
fn flags_each_non_identifier_step_in_a_longer_chain() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().GetParentCell().Reset()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains(".GetParentCell")));
    assert!(diagnostics.iter().any(|d| d.message.contains(".Reset")));
}

fn lint_with_rule(source: &str, enabled: bool) -> Vec<Diagnostic> {
    let mut config = crate::config::Config::default();
    config.rules.member_chain_none_usage = enabled;
    crate::lint(source, &config)
}

#[test]
fn honors_a_line_disable_directive() {
    let diagnostics = lint_with_rule(
        "ScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().Disable() ; @disable member-chain-none-usage\nEndFunction\n",
        true,
    );

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_a_file_disable_directive() {
    let diagnostics = lint_with_rule(
        "; @disable-file member-chain-none-usage\nScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().Disable()\nEndFunction\n",
        true,
    );

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let diagnostics = lint_with_rule(
        "ScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().Disable()\nEndFunction\n",
        false,
    );

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_on_switch_emits_through_lint() {
    let diagnostics = lint_with_rule(
        "ScriptName Example\n\nFunction Test(ObjectReference a)\n    a.GetLinkedRef().Disable()\nEndFunction\n",
        true,
    );

    assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule == RULE));
}

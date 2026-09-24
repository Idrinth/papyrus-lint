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
fn flags_discarded_place_at_me_and_place_actor_at_me_results() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(ObjectReference marker, Form item, ActorBase actor)\n    marker.PlaceAtMe(item)\n    marker.PlaceActorAtMe(actor, 1, None)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[1].line, 4);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.rule == RULE && diagnostic.message.starts_with("[warning]")
    }));
}

#[test]
fn accepts_results_stored_in_declarations_and_assignments() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(ObjectReference marker, Form item, ActorBase actor)\n    ObjectReference placed = marker.PlaceAtMe(item)\n    placed = marker.PlaceActorAtMe(actor, 1, None)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_constant_count_greater_than_one_even_when_result_is_stored() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(ObjectReference marker, Form item)\n    ObjectReference placed = marker.PlaceAtMe(item, 1 + 2)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("creates 3 objects"));
}

#[test]
fn flags_named_count_and_matches_method_names_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(ObjectReference marker, Form item)\n    ObjectReference placed = marker.placeatme(item, aiCount = 2)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_default_single_or_runtime_counts() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(ObjectReference marker, Form item, Int count)\n    ObjectReference one = marker.PlaceAtMe(item)\n    ObjectReference explicitOne = marker.PlaceAtMe(item, 1)\n    ObjectReference unknown = marker.PlaceAtMe(item, count)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_match_bare_calls_or_similar_method_names() {
    let diagnostics = check(
        "ScriptName Example\nFunction Test(Form item)\n    PlaceAtMe(item)\n    Self.PlaceAtMeLater(item)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_and_config_switch_suppress_the_rule() {
    let source = "ScriptName Example\nFunction Test(ObjectReference marker, Form item)\n    marker.PlaceAtMe(item) ; @disable place-at-me\nEndFunction\n";
    assert!(crate::lint(source, &crate::config::Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));

    let source = "; @disable-file place-at-me\nScriptName Example\nFunction Test(ObjectReference marker, Form item)\n    marker.PlaceAtMe(item)\nEndFunction\n";
    assert!(crate::lint(source, &crate::config::Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));

    let source = "ScriptName Example\nFunction Test(ObjectReference marker, Form item)\n    marker.PlaceAtMe(item)\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.place_at_me = false;
    assert!(crate::lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\nFunction Test(\nEndFunction\n").is_empty());
}

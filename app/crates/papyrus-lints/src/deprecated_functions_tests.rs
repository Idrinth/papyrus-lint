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
fn compiled_rules_are_loaded_from_yaml() {
    // Not a specific count: shared/rules/data/deprecated-functions.yaml's
    // entry list is expected to grow (and shrink, as entries move to
    // forbidden-functions.yaml) over time, so this only pins the loader's
    // shape (non-empty, each entry has its expected fields), not its
    // current content.
    assert!(!DEPRECATED_FUNCTIONS.is_empty());
    assert!(DEPRECATED_FUNCTIONS
        .iter()
        .all(|r| !r.script.is_empty() && !r.function.is_empty() && !r.level.is_empty()));
    let rule = DEPRECATED_FUNCTIONS
        .iter()
        .find(|rule| rule.function == "MoveToWhenUnloaded")
        .expect("MoveToWhenUnloaded rule");
    assert_eq!(rule.level, "error");
    assert_eq!(
        rule.replacement,
        Some("MoveTo(akTarget, afXOffset, afYOffset, afZOffset)")
    );
}

#[test]
fn flags_object_method_with_data_message_and_severity() {
    let diagnostics = check("ScriptName Example\nFunction Test(Actor akActor)\n  akActor.ModFavorPoints(1)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (3, 11));
    assert!(diagnostics[0].message.starts_with("[warning] Actor.ModFavorPoints:"));
    assert!(diagnostics[0].message.contains("MakePlayerFriend()"));
}

#[test]
fn global_rule_requires_its_literal_qualifier_case_insensitively() {
    let diagnostics = check("Game.GetSkillLegendaryLevel(\"Smithing\")\ngame.getskilllegendarylevel(\"Smithing\")\nakOther.GetSkillLegendaryLevel(\"Smithing\")\nGetSkillLegendaryLevel(\"Smithing\")\n");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 2);
}

#[test]
fn ignores_comments_strings_and_identifiers_that_are_not_calls() {
    let diagnostics = check("String value = \"Spell.Preload()\" ; Spell.Preload()\nInt Preload = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_suppress_findings() {
    let source = "; @disable-file deprecated-functions\nScriptName Example\nFunction Test(Spell akSpell)\n  akSpell.Preload()\nEndFunction\n";
    let diagnostics = crate::lint(source, &crate::config::Config::default());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_switch_disables_rule_through_dispatch() {
    let source = "ScriptName Example\nFunction Test(Spell akSpell)\n  akSpell.Preload()\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.deprecated_functions = false;

    let diagnostics = crate::lint(source, &config);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

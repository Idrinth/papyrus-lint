use super::*;

#[test]
fn recognizes_deprecated_with_replacement_before_other_annotations() {
    assert!(line_has_deprecated(
        "Function Old() ; @deprecated Use New() @nodiscard @public"
    ));
}

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

struct DeprecatedExternal;

impl crate::external_signatures::ExternalSignatures for DeprecatedExternal {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn deprecated_function(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::Deprecation> {
        (type_name.eq_ignore_ascii_case("LegacyApi")
            && function_name.eq_ignore_ascii_case("OldWay"))
        .then(|| papyrus_parser::ast::Deprecation {
            replacement: Some("NewWay()".to_string()),
            message: "LegacyApi.OldWay: call NewWay() instead".to_string(),
        })
    }
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
        .all(|r| !r.script.is_empty() && !r.function.is_empty() && !r.message.is_empty()));
    let rule = DEPRECATED_FUNCTIONS
        .iter()
        .find(|rule| rule.function == "MoveToWhenUnloaded")
        .expect("MoveToWhenUnloaded rule");
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
fn every_catalogued_deprecation_is_a_warning() {
    let diagnostics = check("ScriptName Example\nFunction Test(ObjectReference target)\n  target.MoveToWhenUnloaded(target)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .starts_with("[warning] ObjectReference.MoveToWhenUnloaded:"));
}

#[test]
fn compiled_object_method_declarations_remain_flagged() {
    let diagnostics = check(
        "ScriptName Actor\nFunction ModFavorPoints(Int aiFavorPoints = 1)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 10));
    assert!(diagnostics[0]
        .message
        .starts_with("[warning] Actor.ModFavorPoints:"));
}

#[test]
fn global_rule_requires_its_literal_qualifier_case_insensitively() {
    let diagnostics = check("Game.GetSkillLegendaryLevel(\"Smithing\")\ngame.getskilllegendarylevel(\"Smithing\")\nakOther.GetSkillLegendaryLevel(\"Smithing\")\nGetSkillLegendaryLevel(\"Smithing\")\n");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 2);
}

#[test]
fn chained_replacement_call_is_not_a_global_qualifier() {
    let diagnostics = check(
        "ScriptName Game\nInt Function GetSkillLegendaryLevel(String asActorValue) Global\n    Return ActorValueInfo.GetActorValueInfoByName(asActorValue).GetSkillLegendaryLevel()\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn chained_call_to_a_same_named_method_does_not_match_its_own_ast_deprecation() {
    // Mirrors what `papyrus-ast-cache`'s build-time catalog merge does for a
    // bundled script whose own declaration matches a `deprecated-functions`
    // entry: the declaration's own `FunctionDecl.deprecation` is set. That
    // must not make a chained call to a same-named method on a different
    // type (whose receiver isn't a plain identifier) look like a recursive
    // self-call to the deprecated declaration.
    let source = "ScriptName Game\nInt Function GetSkillLegendaryLevel(String asActorValue) Global\n    Return ActorValueInfo.GetActorValueInfoByName(asActorValue).GetSkillLegendaryLevel()\nEndFunction\n";
    let mut ast = papyrus_parser::parse(source).unwrap();
    ast.functions[0].deprecation = Some(papyrus_parser::ast::Deprecation {
        replacement: None,
        message: "Game.GetSkillLegendaryLevel: deprecated SKSE wrapper".to_string(),
    });
    let tokens = papyrus_parser::tokenize(source).unwrap();
    let diagnostics = super::check(
        source,
        Some(&ast),
        Some(&tokens),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn emits_structured_external_deprecation_guidance() {
    let source = "ScriptName Example\nLegacyApi Property Api Auto\nFunction Test()\n    Api.OldWay()\nEndFunction\n";
    let ast = papyrus_parser::parse(source).unwrap();
    let tokens = papyrus_parser::tokenize(source).unwrap();
    let diagnostics = super::check(
        source,
        Some(&ast),
        Some(&tokens),
        &crate::config::Config::default(),
        &mut DeprecatedExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "[warning] LegacyApi.OldWay: call NewWay() instead"
    );
}

#[test]
fn ignores_comments_strings_and_identifiers_that_are_not_calls() {
    let diagnostics = check("String value = \"Spell.Preload()\" ; Spell.Preload()\nInt Preload = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_calls_to_a_locally_deprecated_function() {
    let diagnostics = check(
        "ScriptName Example\n\n; @deprecated\nFunction OldWay()\nEndFunction\n\nFunction Test()\n    OldWay()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (8, 5));
    assert_eq!(
        diagnostics[0].message,
        "[warning] Function 'OldWay' is marked deprecated"
    );
}

#[test]
fn includes_a_local_deprecated_directives_note_in_the_message() {
    let diagnostics = check(
        "ScriptName Example\n\n; @deprecated Use New() instead\nFunction OldWay()\nEndFunction\n\nFunction Test()\n    OldWay()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "[warning] Function 'OldWay' is marked deprecated: Use New() instead"
    );
}

#[test]
fn supports_a_trailing_deprecated_directive_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\nFunction OldWay() ; @DePrEcAtEd\nEndFunction\nFunction Test()\n    Self.OldWay()\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn deprecated_word_must_be_bounded_and_inside_a_comment() {
    let diagnostics = check(
        "ScriptName Example\nFunction Similar(String value = \"; @deprecated\") ; @deprecatedSoon\nEndFunction\nFunction Test()\n    Similar()\nEndFunction\n",
    );
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

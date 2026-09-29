use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    check_with_config(source, &crate::config::Config::default())
}

fn check_with_config(source: &str, config: &crate::config::Config) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn compiled_rules_are_loaded_from_yaml() {
    for game in [
        crate::Game::Skyrim,
        crate::Game::Fallout4,
        crate::Game::Starfield,
    ] {
        assert!(!native_methods_for(game).is_empty(), "game: {game:?}");
    }
    assert!(SKYRIM_NATIVE_METHODS
        .iter()
        .any(|rule| rule.object == "Actor" && rule.function == "AddPerk"));
}

#[test]
fn uses_the_catalog_for_the_configured_game() {
    let source = "ScriptName Activator\n\nBool Function IsRadio() Native\n";
    let skyrim = crate::config::Config::default();
    let fallout4 = crate::config::Config {
        game: crate::Game::Fallout4,
        ..crate::config::Config::default()
    };
    let starfield = crate::config::Config {
        game: crate::Game::Starfield,
        ..crate::config::Config::default()
    };

    assert_eq!(check_with_config(source, &skyrim).len(), 1);
    assert!(check_with_config(source, &fallout4).is_empty());
    assert!(check_with_config(source, &starfield).is_empty());
}

#[test]
fn does_not_flag_a_base_game_native_function() {
    let diagnostics = check(
        "ScriptName Actor\n\nFunction AddPerk(Perk akPerk, Bool abForceInform = true) Native\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_base_game_function_case_insensitively() {
    let diagnostics = check(
        "ScriptName actor\n\nFunction addperk(Perk akPerk, Bool abForceInform = true) Native\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_native_function_not_supplied_by_the_base_game() {
    let diagnostics =
        check("ScriptName MyNativeLib\n\nFunction DoSomethingNative(Int aiValue) Global Native\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0]
        .message
        .contains("MyNativeLib.DoSomethingNative"));
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_a_native_function_declared_inside_a_state() {
    let diagnostics = check(
            "ScriptName MyNativeLib\n\nState Busy\n    Function DoSomethingNative(Int aiValue) Global Native\nEndState\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_a_function_with_a_body() {
    let diagnostics =
        check("ScriptName MyScript\n\nFunction DoThing()\n    Debug.Trace(\"hi\")\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_same_named_function_declared_on_an_unrelated_script() {
    let diagnostics = check("ScriptName MyActorHelper\n\nFunction AddPerk(Perk akPerk) Native\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("MyActorHelper.AddPerk"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction DoThing(\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn registry_respects_the_opt_in_rule_switch() {
    let source = "ScriptName MyNativeLib\n\nFunction ExtensionFunction() Native\n";
    let mut config = crate::config::Config::default();

    assert!(crate::lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));

    config.rules.native_function_usage = true;
    assert_eq!(
        crate::lint(source, &config)
            .iter()
            .filter(|diagnostic| diagnostic.rule == RULE)
            .count(),
        1
    );
}

#[test]
fn honors_line_and_file_disable_directives() {
    let mut config = crate::config::Config::default();
    config.rules.native_function_usage = true;
    let line_disabled = "ScriptName MyNativeLib\n\nFunction One() Native ; @disable native-function-usage\nFunction Two() Native\n";
    let file_disabled = "; @disable-file native-function-usage\nScriptName MyNativeLib\n\nFunction One() Native\n";

    let diagnostics = crate::lint(line_disabled, &config);
    let native_diagnostics: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == RULE)
        .collect();
    assert_eq!(native_diagnostics.len(), 1);
    assert_eq!(native_diagnostics[0].line, 4);
    assert!(crate::lint(file_disabled, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

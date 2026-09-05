//! Black-box tests for configuration deserialization and its effect on the
//! crate-level lint and repair entry points.

use papyrus_lints::{
    config::{IdentifierCasing, Indentation},
    lint, repair_filtered, Config,
};

#[test]
fn an_empty_yaml_document_uses_the_complete_default_configuration() {
    let from_yaml: Config = serde_yaml::from_str("").expect("empty config should be valid");

    assert_eq!(from_yaml, Config::default());
}

#[test]
fn a_partial_rules_mapping_preserves_defaults_for_omitted_rules() {
    let config: Config = serde_yaml::from_str("rules:\n  comma_spacing: false\n")
        .expect("partial rules config should be valid");
    let source = "ScriptName Example  \nCall(1,2)\n";
    let diagnostics = lint(source, &config);

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert_eq!(
        repair_filtered(source, &config, Some("comma-spacing")),
        source
    );
}

#[test]
fn yaml_enum_values_deserialize_to_the_documented_variants() {
    let config: Config = serde_yaml::from_str(
        "indentation: space\nidentifier_casing: snake_case\nindentation_width: 2\n",
    )
    .expect("documented enum spellings should deserialize");

    assert_eq!(config.indentation, Indentation::Space);
    assert_eq!(config.identifier_casing, IdentifierCasing::SnakeCase);
    assert_eq!(config.indentation_width, 2);
}

#[test]
fn invalid_yaml_enum_values_are_rejected_instead_of_silently_defaulting() {
    let error = serde_yaml::from_str::<Config>("indentation: spaces\n")
        .expect_err("unknown indentation style should be rejected");

    assert!(error.to_string().contains("unknown variant `spaces`"));
}

#[test]
fn bool_like_int_setting_changes_strict_boolean_results() {
    let source = "ScriptName Example\n\nFunction Run()\n    If 1\n    EndIf\nEndFunction\n";
    let default_diagnostics = lint(source, &Config::default());
    let strict_config: Config =
        serde_yaml::from_str("bool_like_int: false\n").expect("bool_like_int should deserialize");
    let strict_diagnostics = lint(source, &strict_config);

    assert!(default_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "strict-boolean"));
    let strict_boolean: Vec<_> = strict_diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "strict-boolean")
        .collect();
    assert_eq!(strict_boolean.len(), 1);
    assert_eq!((strict_boolean[0].line, strict_boolean[0].column), (4, 5));
}

#[test]
fn config_round_trip_preserves_user_visible_settings_and_rule_switches() {
    let config: Config = serde_yaml::from_str(
        "semicolon: true\nindentation: space\nindentation_width: 8\nidentifier_casing: CONSTANT_CASE\nfail_on_warning: true\nrules:\n  comma_spacing: false\n  magic_numbers: true\n",
    )
    .expect("config fixture should deserialize");

    let serialized = serde_yaml::to_string(&config).expect("config should serialize");
    let round_tripped: Config =
        serde_yaml::from_str(&serialized).expect("serialized config should deserialize");

    assert_eq!(round_tripped, config);
    assert!(round_tripped.semicolon);
    assert!(round_tripped.fail_on_warning);
    assert!(!round_tripped.rules.comma_spacing);
    assert!(round_tripped.rules.magic_numbers);
}

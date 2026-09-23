use super::*;

fn parse(yaml: &str) -> Result<Config, serde_norway::Error> {
    if yaml.trim().is_empty() {
        Ok(Config::default())
    } else {
        serde_norway::from_str(yaml)
    }
}

fn to_yaml(config: &Config) -> Result<String, serde_norway::Error> {
    serde_norway::to_string(config)
}

#[test]
fn empty_document_yields_defaults() {
    assert_eq!(parse("").unwrap(), Config::default());
    assert_eq!(parse("   \n").unwrap(), Config::default());
}

#[test]
fn enabled_ids_hyphenates_names_sorts_and_omits_disabled_rules() {
    let rules = Rules {
        property_sorting: true,     // false by default
        trailing_whitespace: false, // true by default
        ..Rules::default()
    };
    let ids = rules.enabled_ids();

    assert!(!ids.contains(&"trailing-whitespace".to_string()));
    assert!(ids.contains(&"property-sorting".to_string()));
    assert!(ids.contains(&"argument-types".to_string()));
    assert!(ids.is_sorted());
    assert!(ids.iter().all(|id| !id.contains('_')));
}

#[test]
fn default_config_uses_the_generated_rule_defaults() {
    let config = Config::default();
    assert_eq!(config.rules, Rules::default());
}

#[test]
fn parses_individual_rule_overrides() {
    let config = parse("rules:\n  trailing_whitespace: false\n  indentation: false\n").unwrap();

    assert!(!config.rules.trailing_whitespace);
    assert!(!config.rules.indentation);
    // Omitted rule keys still default to enabled.
    assert!(config.rules.comma_spacing);
    assert!(config.rules.semicolon);
}

#[test]
fn rules_round_trip_through_yaml() {
    let config = Config {
        rules: Rules {
            trailing_whitespace: false,
            argument_types: false,
            ..Rules::default()
        },
        ..Config::default()
    };

    let yaml = to_yaml(&config).unwrap();

    assert_eq!(parse(&yaml).unwrap(), config);
}

#[test]
fn defaults_match_documented_default() {
    let config = Config::default();
    assert_eq!(config.game, crate::Game::Skyrim);
    assert!(!config.semicolon);
    assert_eq!(config.indentation, Indentation::Tab);
    assert_eq!(config.indentation_width, 4);
    assert_eq!(config.identifier_casing, IdentifierCasing::PascalCase);
    assert_eq!(config.cyclomatic_complexity_warning, 10);
    assert_eq!(config.cyclomatic_complexity_error, 20);
    assert_eq!(config.type_casing, crate::type_casing::Style::PascalCase);
    assert_eq!(
        config.named_arguments,
        crate::named_arguments::NamedArguments::Never
    );
    assert_eq!(config.min_wait_interval, 0.1);
    assert_eq!(
        config.magic_numbers,
        crate::magic_numbers::MagicNumbers::Loose
    );
    assert!(!config.fail_on_warning);
    assert!(!config.fail_on_info);
    assert!(config.bool_like_int);
    assert!(!config.assume_auto_properties_filled);
}

#[test]
fn parses_game_and_defaults_omitted_game_to_skyrim() {
    assert_eq!(parse("game: skyrim\n").unwrap().game, crate::Game::Skyrim);
    assert_eq!(
        parse("semicolon: true\n").unwrap().game,
        crate::Game::Skyrim
    );
    assert_eq!(
        parse("game: fallout4\n").unwrap().game,
        crate::Game::Fallout4
    );
    assert_eq!(
        parse("game: starfield\n").unwrap().game,
        crate::Game::Starfield
    );
}

#[test]
fn parses_identifier_casing_values() {
    assert_eq!(
        parse("identifier_casing: camelCase\n")
            .unwrap()
            .identifier_casing,
        IdentifierCasing::CamelCase
    );
    assert_eq!(
        parse("identifier_casing: PascalCase\n")
            .unwrap()
            .identifier_casing,
        IdentifierCasing::PascalCase
    );
    assert_eq!(
        parse("identifier_casing: snake_case\n")
            .unwrap()
            .identifier_casing,
        IdentifierCasing::SnakeCase
    );
    assert_eq!(
        parse("identifier_casing: CONSTANT_CASE\n")
            .unwrap()
            .identifier_casing,
        IdentifierCasing::ConstantCase
    );
}

#[test]
fn rejects_unknown_identifier_casing_value() {
    assert!(parse("identifier_casing: kebab-case\n").is_err());
}

#[test]
fn identifier_casing_matches_conforming_names() {
    assert!(IdentifierCasing::CamelCase.matches("myValue"));
    assert!(IdentifierCasing::CamelCase.matches("x"));
    assert!(!IdentifierCasing::CamelCase.matches("MyValue"));
    assert!(!IdentifierCasing::CamelCase.matches("my_value"));

    assert!(IdentifierCasing::PascalCase.matches("MyValue"));
    assert!(!IdentifierCasing::PascalCase.matches("myValue"));
    assert!(!IdentifierCasing::PascalCase.matches("My_Value"));

    assert!(IdentifierCasing::SnakeCase.matches("my_value"));
    assert!(IdentifierCasing::SnakeCase.matches("my_value_2"));
    assert!(!IdentifierCasing::SnakeCase.matches("MyValue"));
    assert!(!IdentifierCasing::SnakeCase.matches("My_Value"));

    assert!(IdentifierCasing::ConstantCase.matches("MY_VALUE"));
    assert!(!IdentifierCasing::ConstantCase.matches("my_value"));
    assert!(!IdentifierCasing::ConstantCase.matches("MyValue"));
}

#[test]
fn identifier_casing_labels_match_their_yaml_values() {
    assert_eq!(IdentifierCasing::CamelCase.label(), "camelCase");
    assert_eq!(IdentifierCasing::PascalCase.label(), "PascalCase");
    assert_eq!(IdentifierCasing::SnakeCase.label(), "snake_case");
    assert_eq!(IdentifierCasing::ConstantCase.label(), "CONSTANT_CASE");
}

#[test]
fn identifier_casing_accepts_an_empty_identifier() {
    for style in [
        IdentifierCasing::CamelCase,
        IdentifierCasing::PascalCase,
        IdentifierCasing::SnakeCase,
        IdentifierCasing::ConstantCase,
    ] {
        assert!(style.matches(""));
    }
}

#[test]
fn identifier_casing_round_trips_through_yaml() {
    for style in [
        IdentifierCasing::CamelCase,
        IdentifierCasing::PascalCase,
        IdentifierCasing::SnakeCase,
        IdentifierCasing::ConstantCase,
    ] {
        let config = Config {
            identifier_casing: style,
            ..Config::default()
        };
        let yaml = to_yaml(&config).unwrap();
        assert_eq!(parse(&yaml).unwrap(), config);
    }
}

#[test]
fn parses_full_config() {
    let config = parse("semicolon: true\nindentation: space\nindentation_width: 2\n").unwrap();
    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
    assert_eq!(config.indentation_width, 2);
}

#[test]
fn missing_keys_fall_back_to_defaults() {
    let config = parse("semicolon: true\n").unwrap();
    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Tab);
    assert_eq!(config.indentation_width, 4);

    let config = parse("indentation: space\n").unwrap();
    assert!(!config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
    assert_eq!(config.indentation_width, 4);
}

#[test]
fn parses_cyclomatic_complexity_thresholds() {
    let config =
        parse("cyclomatic_complexity_warning: 5\ncyclomatic_complexity_error: 15\n").unwrap();
    assert_eq!(config.cyclomatic_complexity_warning, 5);
    assert_eq!(config.cyclomatic_complexity_error, 15);
}

#[test]
fn parses_min_wait_interval() {
    let config = parse("min_wait_interval: 0.25\n").unwrap();
    assert_eq!(config.min_wait_interval, 0.25);
}

#[test]
fn parses_fail_on_flags() {
    let config = parse("fail_on_warning: true\nfail_on_info: true\n").unwrap();
    assert!(config.fail_on_warning);
    assert!(config.fail_on_info);
}

#[test]
fn should_fail_on_honors_fail_on_flags() {
    let error = Diagnostic {
        line: 1,
        column: 1,
        message: "[error] boom".to_string(),
        rule: "some-rule",
    };
    let warning = Diagnostic {
        line: 1,
        column: 1,
        message: "[warning] hmm".to_string(),
        rule: "some-rule",
    };
    let info = Diagnostic {
        line: 1,
        column: 1,
        message: "[info] fyi".to_string(),
        rule: "some-rule",
    };
    let untagged = Diagnostic {
        line: 1,
        column: 1,
        message: "No recognized level prefix here".to_string(),
        rule: "some-rule",
    };

    let default_config = Config::default();
    assert!(default_config.should_fail_on(&error));
    assert!(!default_config.should_fail_on(&warning));
    assert!(!default_config.should_fail_on(&info));
    assert!(default_config.should_fail_on(&untagged));

    let opted_in = Config {
        fail_on_warning: true,
        fail_on_info: true,
        ..Config::default()
    };
    assert!(opted_in.should_fail_on(&error));
    assert!(opted_in.should_fail_on(&warning));
    assert!(opted_in.should_fail_on(&info));
    assert!(opted_in.should_fail_on(&untagged));
}

#[test]
fn parses_bool_like_int() {
    assert!(parse("").unwrap().bool_like_int);
    assert!(!parse("bool_like_int: false\n").unwrap().bool_like_int);
    assert!(parse("bool_like_int: true\n").unwrap().bool_like_int);
}

#[test]
fn bool_like_int_round_trips_through_yaml() {
    let config = Config {
        bool_like_int: false,
        ..Config::default()
    };
    let yaml = to_yaml(&config).unwrap();
    assert_eq!(parse(&yaml).unwrap(), config);
}

#[test]
fn parses_assume_auto_properties_filled() {
    assert!(!parse("").unwrap().assume_auto_properties_filled);
    assert!(
        !parse("assume_auto_properties_filled: false\n")
            .unwrap()
            .assume_auto_properties_filled
    );
    assert!(
        parse("assume_auto_properties_filled: true\n")
            .unwrap()
            .assume_auto_properties_filled
    );
}

#[test]
fn assume_auto_properties_filled_round_trips_through_yaml() {
    let config = Config {
        assume_auto_properties_filled: true,
        ..Config::default()
    };
    let yaml = to_yaml(&config).unwrap();
    assert_eq!(parse(&yaml).unwrap(), config);
}

#[test]
fn rejects_invalid_yaml() {
    assert!(parse("semicolon: [this is not a bool\n").is_err());
}

#[test]
fn rejects_unknown_indentation_value() {
    assert!(parse("indentation: eight-spaces\n").is_err());
}

#[test]
fn parses_type_casing_style() {
    let config = parse("type_casing: camelCase\n").unwrap();
    assert_eq!(config.type_casing, crate::type_casing::Style::CamelCase);

    let config = parse("type_casing: UPPERCASE\n").unwrap();
    assert_eq!(config.type_casing, crate::type_casing::Style::Uppercase);
}

#[test]
fn rejects_unknown_type_casing_value() {
    assert!(parse("type_casing: snake_case\n").is_err());
}

#[test]
fn parses_named_arguments_values() {
    assert_eq!(
        parse("named_arguments: always\n").unwrap().named_arguments,
        crate::named_arguments::NamedArguments::Always
    );
    assert_eq!(
        parse("named_arguments: instead_of_defaults\n")
            .unwrap()
            .named_arguments,
        crate::named_arguments::NamedArguments::InsteadOfDefaults
    );
    assert_eq!(
        parse("named_arguments: never\n").unwrap().named_arguments,
        crate::named_arguments::NamedArguments::Never
    );
}

#[test]
fn rejects_unknown_named_arguments_value() {
    assert!(parse("named_arguments: sometimes\n").is_err());
}

#[test]
fn named_arguments_round_trips_through_yaml() {
    for setting in [
        crate::named_arguments::NamedArguments::Always,
        crate::named_arguments::NamedArguments::InsteadOfDefaults,
        crate::named_arguments::NamedArguments::Never,
    ] {
        let config = Config {
            named_arguments: setting,
            ..Config::default()
        };
        let yaml = to_yaml(&config).unwrap();
        assert_eq!(parse(&yaml).unwrap(), config);
    }
}

#[test]
fn parses_magic_numbers_mode() {
    assert_eq!(
        parse("magic_numbers: loose\n").unwrap().magic_numbers,
        crate::magic_numbers::MagicNumbers::Loose
    );
    assert_eq!(
        parse("magic_numbers: strict\n").unwrap().magic_numbers,
        crate::magic_numbers::MagicNumbers::Strict
    );
}

#[test]
fn rejects_unknown_magic_numbers_value() {
    assert!(parse("magic_numbers: sometimes\n").is_err());
}

#[test]
fn magic_numbers_round_trips_through_yaml() {
    for mode in [
        crate::magic_numbers::MagicNumbers::Loose,
        crate::magic_numbers::MagicNumbers::Strict,
    ] {
        let config = Config {
            magic_numbers: mode,
            ..Config::default()
        };
        let yaml = to_yaml(&config).unwrap();
        assert_eq!(parse(&yaml).unwrap(), config);
    }
}

#[test]
fn semicolon_style_reflects_semicolon_flag() {
    assert_eq!(
        Config {
            semicolon: true,
            ..Config::default()
        }
        .semicolon_style(),
        crate::semicolon::Style::Require
    );
    assert_eq!(
        Config {
            semicolon: false,
            ..Config::default()
        }
        .semicolon_style(),
        crate::semicolon::Style::Forbid
    );
}

#[test]
fn indentation_unit_reflects_indentation_and_width() {
    let config = Config {
        indentation: Indentation::Tab,
        ..Config::default()
    };
    assert_eq!(
        config.indentation_unit(),
        crate::indentation::Indentation::Tabs
    );

    let config = Config {
        indentation: Indentation::Space,
        indentation_width: 2,
        ..Config::default()
    };
    assert_eq!(
        config.indentation_unit(),
        crate::indentation::Indentation::Spaces(2)
    );
}

#[test]
fn indentation_unit_clamps_width_to_valid_range() {
    let config = Config {
        indentation: Indentation::Space,
        indentation_width: 100,
        ..Config::default()
    };
    assert_eq!(
        config.indentation_unit(),
        crate::indentation::Indentation::Spaces(16)
    );

    let config = Config {
        indentation: Indentation::Space,
        indentation_width: 0,
        ..Config::default()
    };
    assert_eq!(
        config.indentation_unit(),
        crate::indentation::Indentation::Spaces(1)
    );
}

#[test]
fn to_yaml_round_trips_through_parse() {
    let config = Config {
        semicolon: true,
        indentation: Indentation::Space,
        indentation_width: 2,
        ..Config::default()
    };

    let yaml = to_yaml(&config).unwrap();

    assert_eq!(parse(&yaml).unwrap(), config);
}

//! Tests for [`crate::repair`], [`crate::repair_filtered`], and
//! [`crate::repair_filtered_by_tag`] — the resolver-less automatic-fix
//! entry points.

use super::super::*;

#[test]
fn combined_repair_applies_comma_spacing_and_trailing_whitespace() {
    let config = Config::default();
    let repaired = repair("Call(1,2)  \r\n", &config);

    assert_eq!(repaired, "Call(1, 2)\r\n");
    assert!(lint(&repaired, &config).is_empty());
}

#[test]
fn combined_repair_closes_whitespace_interrupting_a_chain() {
    let config = Config::default();
    let repaired = repair("SomeProperty . DoThing() .Other()  \r\n", &config);

    assert_eq!(repaired, "SomeProperty.DoThing().Other()\r\n");
    assert!(lint(&repaired, &config).is_empty());
}

#[test]
fn combined_repair_fixes_exclamation_spacing() {
    let config = Config::default();
    let repaired = repair("If !bReady\nEndIf\n", &config);

    assert_eq!(repaired, "If ! bReady\nEndIf\n");
    assert!(lint(&repaired, &config).is_empty());
}

#[test]
fn combined_repair_fixes_configured_type_casing() {
    let config = Config::default();
    let repaired = repair("ScriptName myScript\n", &config);

    assert_eq!(repaired, "ScriptName MyScript\n");
    assert!(lint(&repaired, &config).is_empty());
}

#[test]
fn combined_repair_does_not_fight_trailing_whitespace_over_a_line_ending_negation() {
    // A `!` with nothing but a line ending after it would need a
    // trailing space to satisfy exclamation-spacing, but the trailing
    // whitespace fix runs after it and would strip that space right
    // back off; exclamation-spacing leaves it alone rather than
    // fighting that fix on every `repair()` call.
    let config = Config::default();
    let source = "If !\nEndIf\n";
    let repaired = repair(source, &config);

    assert_eq!(repaired, source);
    assert!(repair(&repaired, &config) == repaired);
}

#[test]
fn repair_honors_configured_semicolon_and_indentation_style() {
    let config = Config {
        semicolon: true,
        indentation: config::Indentation::Space,
        indentation_width: 2,
        ..Config::default()
    };
    let source = "Function Run()\nIf ready\nDoThing()\nEndIf\nEndFunction\n";

    let repaired = repair(source, &config);

    assert_eq!(
        repaired,
        "Function Run();\n  If ready;\n    DoThing();\n  EndIf;\nEndFunction;\n"
    );
}

#[test]
fn repair_skips_disabled_rules() {
    let source = "Call(1,2)  \r\n";
    let config = Config {
        rules: config::Rules {
            trailing_whitespace: false,
            comma_spacing: false,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    assert_eq!(repair(source, &config), source);
}

#[test]
fn repair_skips_chain_whitespace_fix_when_disabled() {
    let source = "SomeProperty . DoThing()\n";
    let config = Config {
        rules: config::Rules {
            chain_whitespace: false,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    assert_eq!(repair(source, &config), source);
}

#[test]
fn repair_skips_exclamation_spacing_fix_when_disabled() {
    let source = "If !bReady\nEndIf\n";
    let config = Config {
        rules: config::Rules {
            exclamation_spacing: false,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    assert_eq!(repair(source, &config), source);
}

#[test]
fn repair_skips_type_casing_fix_when_disabled() {
    let source = "ScriptName myScript\n";
    let config = Config {
        rules: config::Rules {
            type_casing: false,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    assert_eq!(repair(source, &config), source);
}

#[test]
fn repair_filtered_applies_only_the_named_rule() {
    let config = Config::default();
    let source = "Call(1,2)  \r\n";

    let comma_only = repair_filtered(source, &config, Some(comma_spacing::RULE));
    assert_eq!(comma_only, "Call(1, 2)  \r\n");

    let whitespace_only = repair_filtered(source, &config, Some(trailing_whitespace::RULE));
    assert_eq!(whitespace_only, "Call(1,2)\r\n");

    assert_eq!(
        repair_filtered(source, &config, None),
        repair(source, &config)
    );
}

#[test]
fn repair_filtered_matches_nothing_for_an_unknown_rule_id() {
    let config = Config::default();
    let source = "Call(1,2)  \r\n";

    assert_eq!(
        repair_filtered(source, &config, Some("made-up-rule")),
        source
    );
}

#[test]
fn repair_filtered_by_tag_applies_all_matching_fixes_only() {
    let config = Config::default();
    // Comma spacing and trailing whitespace are style fixes, while
    // GetValueInt is covered by the performance-tagged slow-functions fix.
    let source =
        "Function Read(GlobalVariable value)  \n\tFoo(1,2)\n\tvalue.GetValueInt()\nEndFunction\n";

    let repaired = repair_filtered_by_tag(source, &config, Some("style"));

    assert_eq!(
        repaired,
        "Function Read(GlobalVariable value)\n\tFoo(1, 2)\n\tvalue.GetValueInt()\nEndFunction\n"
    );
}

#[test]
fn repair_filtered_by_tag_matches_case_insensitively() {
    let source = "GlobalVariable value\nvalue.GetValueInt()\n";

    assert_eq!(
        repair_filtered_by_tag(source, &Config::default(), Some("PERFORMANCE")),
        "GlobalVariable value\nvalue.GetValue() As Int\n"
    );
}

#[test]
fn repair_filtered_by_tag_with_none_matches_combined_repair() {
    let source = "Call(1,2)  \r\n";
    let config = Config::default();

    assert_eq!(
        repair_filtered_by_tag(source, &config, None),
        repair(source, &config)
    );
}

#[test]
fn repair_filtered_by_tag_matches_nothing_for_an_unknown_tag() {
    let source = "Call(1,2)  \r\n";

    assert_eq!(
        repair_filtered_by_tag(source, &Config::default(), Some("made-up-tag")),
        source
    );
}

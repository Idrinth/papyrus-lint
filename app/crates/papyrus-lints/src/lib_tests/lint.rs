//! Tests for [`crate::lint`], [`crate::lint_with_external_arguments`], and
//! [`crate::lint_with_external_arguments_and_extra_diagnostics`]: config
//! off-switches, `@disable` / `@disable-file`, unused-disable, and extra
//! diagnostics merged in before those checks.

use super::super::*;

#[test]
fn lint_skips_disabled_rules() {
    let source = "Foo(1,2)   \n";
    let config = Config::default();
    let baseline = lint(source, &config);
    assert_eq!(baseline.len(), 2);

    let config = Config {
        rules: config::Rules {
            trailing_whitespace: false,
            comma_spacing: false,
            ..config::Rules::default()
        },
        ..Config::default()
    };
    assert!(lint(source, &config).is_empty());
}

#[test]
fn lint_honors_disable_comments_on_the_flagged_line_only() {
    let source = "Foo(1,2)   \nBar(3,4)   \n";
    let config = Config::default();
    let baseline = lint(source, &config);
    assert_eq!(baseline.len(), 4);

    let source = "Foo(1,2) ; @disable comma-spacing \nBar(3,4)   \n";
    let diagnostics = lint(source, &config);

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics
        .iter()
        .all(|d| !(d.line == 1 && d.rule == comma_spacing::RULE)));
    assert!(diagnostics
        .iter()
        .any(|d| d.line == 2 && d.rule == comma_spacing::RULE));
}

#[test]
fn lint_bare_disable_comment_suppresses_every_rule_on_the_line() {
    let source = "Foo(1,2)   ; @disable\n";
    assert!(lint(source, &Config::default()).is_empty());
}

#[test]
fn lint_honors_disable_file_comments_on_every_line() {
    let source = "Foo(1,2) ; @disable-file comma-spacing\nBar(3,4)   \n";
    let config = Config::default();

    let diagnostics = lint(source, &config);

    assert!(diagnostics.iter().all(|d| d.rule != comma_spacing::RULE));
    assert!(diagnostics
        .iter()
        .any(|d| d.line == 2 && d.rule == trailing_whitespace::RULE));
}

#[test]
fn lint_bare_disable_file_comment_suppresses_every_rule_in_the_file() {
    let source = "Foo(1,2)   ; @disable-file\nBar(3,4)   \n";
    assert!(lint(source, &Config::default()).is_empty());
}

#[test]
fn lint_disable_file_directive_need_not_be_on_the_first_line() {
    let source = "Foo(1,2)\nBar(3,4) ; @disable-file comma-spacing\n";
    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics.iter().all(|d| d.rule != comma_spacing::RULE));
}

#[test]
fn unused_disable_defaults_to_off() {
    let diagnostics = lint("Foo(1, 2) ; @disable made-up-rule\n", &Config::default());

    assert!(diagnostics.iter().all(|d| d.rule != unused_disable::RULE));
}

#[test]
fn unused_disable_reports_unknown_and_untriggered_rule_ids() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };
    let source = "Foo(1,2) ; @disable comma-spacing, made-up, float-to-int\n";

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.rule == unused_disable::RULE)
        .collect();

    assert_eq!(unused.len(), 2);
    assert!(unused
        .iter()
        .any(|d| d.message.contains("made-up") && d.message.contains("unknown")));
    assert!(unused
        .iter()
        .any(|d| d.message.contains("float-to-int") && d.message.contains("does not produce")));
    assert!(unused.iter().all(|d| !d.message.contains("comma-spacing")));
}

#[test]
fn unused_disable_reports_unknown_and_untriggered_disable_file_rule_ids() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };
    let source = "Foo(1,2) ; @disable-file comma-spacing, made-up, float-to-int\n";

    let diagnostics = lint(source, &config);
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.rule == unused_disable::RULE)
        .collect();

    assert_eq!(unused.len(), 2);
    assert!(unused
        .iter()
        .any(|d| d.message.contains("made-up") && d.message.contains("unknown")));
    assert!(unused
        .iter()
        .any(|d| d.message.contains("float-to-int") && d.message.contains("does not produce")));
    assert!(unused.iter().all(|d| !d.message.contains("comma-spacing")));
}

#[test]
fn unused_disable_reports_a_bare_disable_file_directive_on_a_clean_file() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    let diagnostics = lint("; @disable-file\n", &config);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, unused_disable::RULE);
    assert!(diagnostics[0].message.contains("@disable-file"));
}

#[test]
fn unused_disable_reports_a_bare_directive_on_a_clean_line() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    let diagnostics = lint("; @disable\n", &config);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, unused_disable::RULE);
    assert_eq!(diagnostics[0].column, 3);
}

#[test]
fn unused_disable_columns_count_characters_instead_of_utf8_bytes() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };

    let diagnostics = lint("String value = \"é\" ; @disable unknown\n", &config);
    let diagnostic = diagnostics
        .iter()
        .find(|d| d.rule == unused_disable::RULE)
        .unwrap();

    assert_eq!(diagnostic.column, 31);
}

#[test]
fn extra_diagnostics_disabled_by_a_directive_are_not_reported_as_unused() {
    // Simulates `papyrus-lint-core`'s path-dependent project diagnostics
    // (e.g. `stale-compiled-output`), which are computed outside this
    // crate and merged in via `extra_diagnostics` rather than appended
    // to `lint_with_external_arguments`'s own result — see that
    // function's docs and https://github.com/Idrinth/papyrus-lint/issues/772.
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };
    let source = "; @disable stale-compiled-output\n";
    let extra = vec![Diagnostic {
        line: 1,
        column: 1,
        message: "[info] stale".into(),
        rule: "stale-compiled-output",
    }];

    let diagnostics = lint_with_external_arguments_and_extra_diagnostics(
        source,
        &config,
        &mut external_signatures::NoExternalSignatures,
        extra,
    );

    assert!(diagnostics
        .iter()
        .all(|d| d.rule != "stale-compiled-output"));
    assert!(diagnostics.iter().all(|d| d.rule != unused_disable::RULE));
}

#[test]
fn extra_diagnostics_not_covered_by_a_directive_still_report_it_as_unused() {
    let config = Config {
        rules: config::Rules {
            unused_disable: true,
            ..config::Rules::default()
        },
        ..Config::default()
    };
    let source = "; @disable-file stale-compiled-output\n";

    let diagnostics = lint_with_external_arguments_and_extra_diagnostics(
        source,
        &config,
        &mut external_signatures::NoExternalSignatures,
        Vec::new(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, unused_disable::RULE);
    assert!(diagnostics[0].message.contains("stale-compiled-output"));
}

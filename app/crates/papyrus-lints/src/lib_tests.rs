use super::*;

#[test]
fn diagnostic_level_parses_the_leading_tag() {
    let tagged = |message: &str| Diagnostic {
        line: 1,
        column: 1,
        message: message.to_string(),
        rule: "some-rule",
    };

    assert_eq!(tagged("[error] boom").level(), "error");
    assert_eq!(tagged("[warning] hmm").level(), "warning");
    assert_eq!(tagged("[info] fyi").level(), "info");
    assert_eq!(tagged("No recognized level prefix here").level(), "error");
}

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
        &mut argument_types::NoExternalSignatures,
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
        &mut argument_types::NoExternalSignatures,
        Vec::new(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, unused_disable::RULE);
    assert!(diagnostics[0].message.contains("stale-compiled-output"));
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

#[test]
fn restrict_to_line_keeps_only_the_target_line_changed() {
    let original = "Call(1,2)\nCall(3,4)\nCall(5,6)\n";
    let repaired = repair_filtered(original, &Config::default(), Some(comma_spacing::RULE));

    let restricted = restrict_to_line(original, &repaired, 2).unwrap();

    assert_eq!(restricted, "Call(1,2)\nCall(3, 4)\nCall(5,6)\n");
}

#[test]
fn restrict_to_line_returns_none_when_line_count_changes() {
    let original = "Call(1,2)\n";
    let repaired = "Call(1,2)\nCall(3,4)\n";

    assert_eq!(restrict_to_line(original, repaired, 1), None);
}

#[test]
fn restrict_to_line_out_of_range_leaves_original_unchanged() {
    let original = "Call(1,2)\nCall(3,4)\n";
    let repaired = "Call(1, 2)\nCall(3, 4)\n";

    assert_eq!(
        restrict_to_line(original, repaired, 0).as_deref(),
        Some(original)
    );
    assert_eq!(
        restrict_to_line(original, repaired, 4).as_deref(),
        Some(original)
    );
}

#[test]
fn restrict_to_line_can_replace_the_trailing_empty_line() {
    let original = "Call(1,2)\n";
    let repaired = "Call(1,2)\nreplacement";

    assert_eq!(
        restrict_to_line(original, repaired, 2).as_deref(),
        Some("Call(1,2)\nreplacement")
    );
}

#[test]
fn repaired_line_returns_just_the_target_line_after_the_fix() {
    let source = "Call(1,2)\nCall(3,4)\nCall(5,6)\n";
    let config = Config::default();

    assert_eq!(
        repaired_line(source, &config, comma_spacing::RULE, 2).as_deref(),
        Some("Call(3, 4)")
    );
}

#[test]
fn repaired_line_is_none_when_the_rule_does_not_change_the_source_at_all() {
    let source = "Call(1, 2)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 1),
        None
    );
}

#[test]
fn repaired_line_is_none_when_the_fix_shifts_the_line_count() {
    let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n";
    let config = config_with(|c| c.rules.property_sorting = true);

    assert_eq!(
        repaired_line(source, &config, property_sorting::RULE, 3),
        None
    );
}

#[test]
fn repaired_line_is_none_for_an_out_of_range_line() {
    let source = "Call(1,2)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 99),
        None
    );
}

#[test]
fn repaired_line_is_none_when_the_fix_touches_other_lines_but_not_the_target_one() {
    let source = "Call(1,2)\nCall(3, 4)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 2),
        None
    );
}

#[test]
fn add_disable_comment_suppresses_the_named_rule_on_the_target_line() {
    let source = "Call(1,2)  \n";
    let config = Config::default();
    assert!(!lint(source, &config).is_empty());

    let updated = add_disable_comment(source, 1, &[comma_spacing::RULE.to_string()]);
    let remaining: Vec<_> = lint(&updated, &config)
        .into_iter()
        .filter(|finding| finding.rule == comma_spacing::RULE)
        .collect();

    assert!(remaining.is_empty());
}

#[test]
fn is_disabled_exposes_line_and_file_directive_checks() {
    let source = concat!(
        "Call(1,2) ; @disable comma-spacing\n",
        "Call(3,4)\n",
        "; @disable-file trailing-whitespace\n",
    );

    assert!(is_disabled(source, 1, comma_spacing::RULE));
    assert!(!is_disabled(source, 2, comma_spacing::RULE));
    assert!(is_disabled(source, 2, trailing_whitespace::RULE));
    assert!(!is_disabled(source, 2, semicolon::RULE));
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

fn config_with(tweak: impl FnOnce(&mut Config)) -> Config {
    let mut config = Config::default();
    tweak(&mut config);
    config
}

/// `lint_with_external_arguments` gates each ruleset behind its own `if
/// rules.<field>` check (see [`config::Rules`]). This walks every
/// ruleset except `function_override`, one at a time, confirming that:
/// (a) its own flag being on lets it fire on a source crafted to
/// trigger it, and (b) flipping only that flag off suppresses that
/// rule's diagnostics. `function_override` can never fire through bare
/// `lint()` (it always needs an `external` resolver for the script's
/// `Extends` chain), so its gate is exercised separately by
/// `function_override_flag_gates_only_its_own_lint`, below — a gate
/// accidentally wired to the wrong `Rules` field, or hardcoded to
/// always run, would fail
/// here even though it wouldn't fail any other existing test.
#[test]
fn each_rule_flag_gates_only_its_own_lint() {
    let many_states_source: String = {
        let mut source = "ScriptName Example\n\n".to_string();
        for index in 0..128 {
            source.push_str(&format!("State State{index}\nEndState\n"));
        }
        source
    };
    let cases: Vec<(&str, &str, Config, Config)> = vec![
            (
                "ScriptName Example  \n",
                trailing_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.trailing_whitespace = false),
            ),
            (
                "Function Add(Int left,Int right)\n  Use(Add(1,2),3)\nEndFunction\n",
                comma_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.comma_spacing = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
                forbidden_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.forbidden_functions = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
                slow_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.slow_functions = false),
            ),
            (
                "Function Test()\n  GetValue()\nEndFunction\n",
                unused_getter::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_getter = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
                unused_property::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_property = false),
            ),
            (
                // Default config forbids trailing semicolons, so a semicolon here violates it.
                "ScriptName Example\n\nInt value = 1;\n",
                semicolon::RULE,
                Config::default(),
                config_with(|c| c.rules.semicolon = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int x = 1.5\nEndFunction\n",
                float_int_conversion::RULE,
                Config::default(),
                config_with(|c| c.rules.float_int_conversion = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2\nEndFunction\n",
                int_division_to_float::RULE,
                Config::default(),
                config_with(|c| c.rules.int_division_to_float = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\nEndFunction\n",
                strict_boolean::RULE,
                Config::default(),
                config_with(|c| c.rules.strict_boolean = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(1)\nEndFunction\n",
                argument_types::RULE,
                Config::default(),
                config_with(|c| c.rules.argument_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
                numeric_comparison::RULE,
                Config::default(),
                config_with(|c| c.rules.numeric_comparison = false),
            ),
            (
                // Default config expects tab indentation; this uses spaces instead.
                "Function Run()\n  If ready\nDoThing()\nEndIf\nEndFunction\n",
                indentation::RULE,
                Config::default(),
                config_with(|c| c.rules.indentation = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                cyclomatic_complexity::RULE,
                // The default warning threshold (10) wouldn't flag this trivial
                // function, so lower it — independent of the `rules` flag under test.
                config_with(|c| c.cyclomatic_complexity_warning = 0),
                config_with(|c| {
                    c.cyclomatic_complexity_warning = 0;
                    c.rules.cyclomatic_complexity = false;
                }),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\nEndFunction\n",
                unreachable_statement::RULE,
                Config::default(),
                config_with(|c| c.rules.unreachable_statement = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n",
                return_types::RULE,
                Config::default(),
                config_with(|c| c.rules.return_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                unused_local_variable::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_local_variable = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
                none_form_usage::RULE,
                Config::default(),
                config_with(|c| c.rules.none_form_usage = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
                variable_used_before_assignment::RULE,
                Config::default(),
                config_with(|c| c.rules.variable_used_before_assignment = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\n    Debug.Trace(MyValue)\nEndFunction\n",
                local_variable_shadowing::RULE,
                Config::default(),
                config_with(|c| c.rules.local_variable_shadowing = false),
            ),
            (
                "SomeProperty . DoThing()\n",
                chain_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.chain_whitespace = false),
            ),
            (
                "If !bReady\nEndIf\n",
                exclamation_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.exclamation_spacing = false),
            ),
            (
                "If a==b\nEndIf\n",
                operator_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.operator_spacing = false),
            ),
            (
                "a=b\n",
                assignment_operator_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.assignment_operator_spacing = false),
            ),
            (
                "ScriptName Example\n\nInt Property bad_name = 1 Auto\n",
                identifier_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.identifier_casing = false),
            ),
            (
                "ScriptName myExample\n",
                type_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.type_casing = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
                named_arguments::RULE,
                config_with(|c| c.named_arguments = named_arguments::NamedArguments::Always),
                config_with(|c| {
                    c.named_arguments = named_arguments::NamedArguments::Always;
                    c.rules.named_arguments = false;
                }),
            ),
            (
                "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
                property_sorting::RULE,
                config_with(|c| c.rules.property_sorting = true),
                config_with(|c| c.rules.property_sorting = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Int i = 1\nEndFunction\n",
                explicit_return::RULE,
                Config::default(),
                config_with(|c| c.rules.explicit_return = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
                unchecked_form_parameter::RULE,
                config_with(|c| c.rules.unchecked_form_parameter = true),
                config_with(|c| c.rules.unchecked_form_parameter = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
                unchecked_array_element::RULE,
                config_with(|c| c.rules.unchecked_array_element = true),
                config_with(|c| c.rules.unchecked_array_element = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).GetActorValue(\"Health\")\nEndFunction\n",
                unchecked_cast::RULE,
                Config::default(),
                config_with(|c| c.rules.unchecked_cast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as Actor)\nEndFunction\n",
                useless_downcast::RULE,
                Config::default(),
                config_with(|c| c.rules.useless_downcast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / 0\nEndFunction\n",
                division_by_zero::RULE,
                Config::default(),
                config_with(|c| c.rules.division_by_zero = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
                empty_body::RULE,
                Config::default(),
                config_with(|c| c.rules.empty_body = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.01)\nEndFunction\n",
                short_wait_interval::RULE,
                Config::default(),
                config_with(|c| c.rules.short_wait_interval = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
                formid_hex_notation::RULE,
                Config::default(),
                config_with(|c| c.rules.formid_hex_notation = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n",
                state_function_signature::RULE,
                Config::default(),
                config_with(|c| c.rules.state_function_signature = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
                goto_state::RULE,
                Config::default(),
                config_with(|c| c.rules.goto_state = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
                get_state_comparison::RULE,
                Config::default(),
                config_with(|c| c.rules.get_state_comparison = false),
            ),
            (
                many_states_source.as_str(),
                state_count::TOO_MANY_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.too_many_states = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
                magic_numbers::RULE,
                config_with(|c| c.rules.magic_numbers = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n",
                state_count::MULTIPLE_AUTO_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.multiple_auto_states = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    ElseIf gv.GetValue() == 2.0\n    EndIf\nEndFunction\n",
                repeated_getvalue::RULE,
                config_with(|c| c.rules.repeated_getvalue = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 2.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
                global_variable_setvalue::RULE,
                config_with(|c| c.rules.global_variable_setvalue = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
                global_variable_increment::RULE,
                Config::default(),
                config_with(|c| c.rules.global_variable_increment = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
                setvalue_in_loop::RULE,
                Config::default(),
                config_with(|c| c.rules.setvalue_in_loop = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int n = 5\n    While n < 10\n        Debug.Trace(\"y\")\n    EndWhile\nEndFunction\n",
                invariant_loop_condition::RULE,
                Config::default(),
                config_with(|c| c.rules.invariant_loop_condition = false),
            ),
            (
                "ScriptName Example\n\nInt Property Example Auto\n",
                script_name_collision::RULE,
                Config::default(),
                config_with(|c| c.rules.script_name_collision = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[5] = 1\nEndFunction\n",
                array_bounds::RULE,
                Config::default(),
                config_with(|c| c.rules.array_bounds = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[200]\nEndFunction\n",
                array_size_range::RULE,
                Config::default(),
                config_with(|c| c.rules.array_size_range = false),
            ),
            (
                "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
                readonly_property_write::RULE,
                Config::default(),
                config_with(|c| c.rules.readonly_property_write = false),
            ),
            (
                "ScriptName Example\n\nInt Property Count Auto\n",
                default_property_value::RULE,
                config_with(|c| c.rules.default_property_value = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Test()\nEndFunction\n",
                unguarded_self_recursion::RULE,
                Config::default(),
                config_with(|c| c.rules.unguarded_self_recursion = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = a\nEndFunction\n",
                self_assignment::RULE,
                Config::default(),
                config_with(|c| c.rules.self_assignment = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor, Form item)\n    Debug.Trace(akActor.RemoveItem(item, 1))\nEndFunction\n",
                debug_side_effects::RULE,
                Config::default(),
                config_with(|c| c.rules.debug_side_effects = false),
            ),
            (
                "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n",
                unnecessary_function::RULE,
                Config::default(),
                config_with(|c| c.rules.unnecessary_function = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.GetActorValue(\"NotARealActorValue\")\nEndFunction\n",
                actor_value::RULE,
                config_with(|c| c.rules.unknown_actor_value = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
                repeated_setoutfit::RULE,
                Config::default(),
                config_with(|c| c.rules.repeated_setoutfit = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\nEndFunction\n",
                missing_doc_comment::RULE,
                config_with(|c| c.rules.missing_doc_comment = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(10, 5)\nEndFunction\n",
                invalid_random_range::RULE,
                Config::default(),
                config_with(|c| c.rules.invalid_random_range = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Float a, Float b)\n    If a == b\n    EndIf\nEndFunction\n",
                float_equality::RULE,
                config_with(|c| c.rules.float_equality = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n",
                missing_update_handler::RULE,
                config_with(|c| c.rules.missing_update_handler = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nEvent OnActivate()\nEndEvent\n",
                event_signature::RULE,
                config_with(|c| c.rules.event_signature_mismatch = true),
                Config::default(),
            ),
        ];

    for (source, rule, enabled_config, disabled_config) in cases {
        let baseline = lint(source, &enabled_config);
        assert!(
            baseline.iter().any(|d| d.rule == rule),
            "expected rule {rule:?} to fire on {source:?}, got {baseline:?}"
        );

        let with_rule_disabled = lint(source, &disabled_config);
        assert!(
            with_rule_disabled.iter().all(|d| d.rule != rule),
            "disabling {rule:?} should suppress its own diagnostics, got {with_rule_disabled:?}"
        );
    }
}

struct FakeExternalWithParentFunction;

impl argument_types::ExternalSignatures for FakeExternalWithParentFunction {
    fn lookup(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(Vec::new())
        } else {
            None
        }
    }
}

/// See the note on `each_rule_flag_gates_only_its_own_lint` above:
/// `function_override` needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.function_override`
/// gate is checked here instead of in that loop.
#[test]
fn function_override_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithParentFunction,
    );
    assert!(enabled.iter().any(|d| d.rule == function_override::RULE));

    let disabled_config = config_with(|c| c.rules.function_override = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithParentFunction,
    );
    assert!(disabled.iter().all(|d| d.rule != function_override::RULE));
}

struct FakeExternalWithMissingScript;

impl argument_types::ExternalSignatures for FakeExternalWithMissingScript {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("KnownScript")
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `unresolved_script` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.unresolved_script` gate is checked here instead of in the
/// main loop.
#[test]
fn unresolved_script_flag_gates_only_its_own_lint() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    MissingScript.DoThing()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithMissingScript,
    );
    assert!(enabled.iter().any(|d| d.rule == unresolved_script::RULE));

    let disabled_config = config_with(|c| c.rules.unresolved_script = false);
    let disabled =
        lint_with_external_arguments(source, &disabled_config, &mut FakeExternalWithMissingScript);
    assert!(disabled.iter().all(|d| d.rule != unresolved_script::RULE));
}

struct FakeExternalWithCircularProperty;

impl argument_types::ExternalSignatures for FakeExternalWithCircularProperty {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn property_types(&mut self, type_name: &str) -> Vec<String> {
        if type_name.eq_ignore_ascii_case("B") {
            vec!["Example".to_string()]
        } else {
            Vec::new()
        }
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `circular_dependency` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.circular_dependency` gate is checked here instead of in the
/// main loop. It also defaults to `false` (see `config::Rules`), unlike
/// every rule the main loop covers, so both configs here are built from
/// `config_with` rather than one being `Config::default()`.
#[test]
fn circular_dependency_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nB Property Little Auto\n";

    let enabled_config = config_with(|c| c.rules.circular_dependency = true);
    let enabled = lint_with_external_arguments(
        source,
        &enabled_config,
        &mut FakeExternalWithCircularProperty,
    );
    assert!(enabled.iter().any(|d| d.rule == circular_dependency::RULE));

    let disabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithCircularProperty,
    );
    assert!(disabled.iter().all(|d| d.rule != circular_dependency::RULE));
}

struct FakeExternalWithNonGlobalFunction;

impl argument_types::ExternalSignatures for FakeExternalWithNonGlobalFunction {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("NotGlobal")
        {
            Some(false)
        } else {
            None
        }
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `non_global_function_call` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.non_global_function_call` gate is checked here instead of in
/// the main loop.
#[test]
fn non_global_function_call_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nFunction Test()\n    MyScript.NotGlobal()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithNonGlobalFunction,
    );
    assert!(enabled
        .iter()
        .any(|d| d.rule == non_global_function_call::RULE));

    let disabled_config = config_with(|c| c.rules.non_global_function_call = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithNonGlobalFunction,
    );
    assert!(disabled
        .iter()
        .all(|d| d.rule != non_global_function_call::RULE));
}

struct FakeExternalWithGlobalFunction;

impl argument_types::ExternalSignatures for FakeExternalWithGlobalFunction {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("IsGlobal")
        {
            Some(true)
        } else {
            None
        }
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `static_function_call_via_instance` also needs
/// `lint_with_external_arguments`'s `external` resolver to ever fire, so
/// its own `rules.static_function_call_via_instance` gate is checked
/// here instead of in the main loop.
#[test]
fn static_function_call_via_instance_flag_gates_only_its_own_lint() {
    let source =
        "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.IsGlobal()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithGlobalFunction,
    );
    assert!(enabled
        .iter()
        .any(|d| d.rule == static_function_call_via_instance::RULE));

    let disabled_config = config_with(|c| c.rules.static_function_call_via_instance = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithGlobalFunction,
    );
    assert!(disabled
        .iter()
        .all(|d| d.rule != static_function_call_via_instance::RULE));
}

struct FakeExternalWithRenamedParentParam;

impl argument_types::ExternalSignatures for FakeExternalWithRenamedParentParam {
    fn lookup(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(vec![argument_types::ParamInfo {
                name: "akTarget".to_string(),
                type_name: papyrus_parser::ast::TypeName {
                    name: "ObjectReference".to_string(),
                    is_array: false,
                },
            }])
        } else {
            None
        }
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `argument_naming` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.argument_naming`
/// gate is checked here instead of in the main loop.
#[test]
fn argument_naming_flag_gates_only_its_own_lint() {
    let source =
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithRenamedParentParam,
    );
    assert!(enabled.iter().any(|d| d.rule == argument_naming::RULE));

    let disabled_config = config_with(|c| c.rules.argument_naming = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithRenamedParentParam,
    );
    assert!(disabled.iter().all(|d| d.rule != argument_naming::RULE));
}

struct FakeExternalWithUnrelatedAncestry;

impl argument_types::ExternalSignatures for FakeExternalWithUnrelatedAncestry {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Armor") || type_name.eq_ignore_ascii_case("Weapon")
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `impossible_cast` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire (see `impossible_cast`'s module
/// docs), so its own `rules.impossible_cast` gate is checked here
/// instead of in the main loop.
#[test]
fn impossible_cast_flag_gates_only_its_own_lint() {
    let source =
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnrelatedAncestry,
    );
    assert!(enabled.iter().any(|d| d.rule == impossible_cast::RULE));

    let disabled_config = config_with(|c| c.rules.impossible_cast = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithUnrelatedAncestry,
    );
    assert!(disabled.iter().all(|d| d.rule != impossible_cast::RULE));
}

struct FakeExternalWithUnusedImport;

impl argument_types::ExternalSignatures for FakeExternalWithUnusedImport {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<argument_types::ParamInfo>> {
        None
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Helpers")
    }
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `unused_import` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.unused_import`
/// gate is checked here instead of in the main loop.
#[test]
fn unused_import_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
    );
    assert!(enabled.iter().any(|d| d.rule == unused_import::RULE));

    let disabled_config = config_with(|c| c.rules.unused_import = false);
    let disabled =
        lint_with_external_arguments(source, &disabled_config, &mut FakeExternalWithUnusedImport);
    assert!(disabled.iter().all(|d| d.rule != unused_import::RULE));
}

#[test]
fn repair_with_external_arguments_removes_an_unused_import_resolved_through_external() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let repaired = repair_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
    );

    assert_eq!(
        repaired,
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert_eq!(
        repair(source, &Config::default()),
        source,
        "the plain, resolver-less repair must remain a no-op for unused-import"
    );
}

#[test]
fn repair_filtered_with_external_arguments_only_removes_the_named_rule() {
    let source =
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n";

    let unused_import_only = repair_filtered_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(unused_import::RULE),
    );
    assert_eq!(
        unused_import_only,
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );

    let comma_only = repair_filtered_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
    );
    assert_eq!(
        comma_only,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1, 2)\nEndFunction\n"
    );
}

#[test]
fn repair_filtered_by_tag_with_external_arguments_matches_unused_imports_own_tag() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let maintainability = repair_filtered_by_tag_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some("maintainability"),
    );
    assert_eq!(
        maintainability,
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );

    let style_only = repair_filtered_by_tag_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some("style"),
    );
    assert_eq!(style_only, source);
}

#[test]
fn repair_with_external_arguments_skips_unused_import_when_its_rule_is_disabled() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";
    let disabled_config = config_with(|c| c.rules.unused_import = false);

    assert_eq!(
        repair_with_external_arguments(source, &disabled_config, &mut FakeExternalWithUnusedImport),
        source
    );
}

#[test]
fn repair_selected_prefers_a_tag_filter_over_a_rule_filter() {
    let source = "Call(1,2)  \n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
        Some("style"),
        None,
    )
    .unwrap();

    assert_eq!(repaired, "Call(1, 2)\n");
}

#[test]
fn repair_selected_can_restrict_a_named_fix_to_one_line() {
    let source = "Call(1,2)\nCall(3,4)\n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
        None,
        Some(2),
    )
    .unwrap();

    assert_eq!(repaired, "Call(1,2)\nCall(3, 4)\n");
}

#[test]
fn repair_selected_returns_none_for_a_line_count_shifting_fix() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(unused_import::RULE),
        None,
        Some(3),
    );

    assert_eq!(repaired, None);
}

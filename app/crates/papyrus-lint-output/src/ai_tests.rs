use super::*;

fn diagnostic(rule: &str, level: &'static str) -> JsonDiagnostic {
    JsonDiagnostic {
        line: 1,
        column: 1,
        rule: rule.to_string(),
        level,
        message: "example".to_string(),
        doc_url: None,
    }
}

#[test]
fn unix_timestamps_are_formatted_as_utc_rfc3339() {
    assert_eq!(format_unix_timestamp(0, 0), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        format_unix_timestamp(1_709_251_199, 42),
        "2024-02-29T23:59:59.042Z"
    );
}

#[test]
fn unix_timestamps_handle_calendar_boundaries_and_millisecond_padding() {
    assert_eq!(
        format_unix_timestamp(31_535_999, 7),
        "1970-12-31T23:59:59.007Z"
    );
    assert_eq!(
        format_unix_timestamp(31_536_000, 70),
        "1971-01-01T00:00:00.070Z"
    );
    assert_eq!(
        format_unix_timestamp(951_782_400, 999),
        "2000-02-29T00:00:00.999Z"
    );
}

#[test]
fn severity_counts_tallies_each_known_level_and_ignores_unknown_levels() {
    let diagnostics = [
        diagnostic("first-rule", "error"),
        diagnostic("second-rule", "warning"),
        diagnostic("third-rule", "info"),
        diagnostic("future-rule", "notice"),
    ];

    let counts = severity_counts(&diagnostics);

    assert_eq!(counts.errors, 1);
    assert_eq!(counts.warnings, 1);
    assert_eq!(counts.info, 1);
}

#[test]
fn rule_counts_aggregates_duplicates_in_sorted_rule_order() {
    let diagnostics = [
        diagnostic("z-rule", "warning"),
        diagnostic("a-rule", "warning"),
        diagnostic("z-rule", "warning"),
    ];

    let counts = rule_counts(&diagnostics);

    assert_eq!(
        counts.keys().cloned().collect::<Vec<_>>(),
        ["a-rule".to_string(), "z-rule".to_string()]
    );
    assert_eq!(counts["a-rule"], 1);
    assert_eq!(counts["z-rule"], 2);
}

#[test]
fn ai_configuration_replaces_rule_flags_with_enabled_rule_ids() {
    let config = papyrus_lints::Config::default();

    let value = ai_configuration(&config);
    let object = value
        .as_object()
        .expect("configuration should be an object");
    let enabled = object["enabled_rules"]
        .as_array()
        .expect("enabled_rules should be an array");

    assert!(!object.contains_key("rules"));
    assert!(enabled.contains(&serde_json::json!("trailing-whitespace")));
    assert!(!enabled.contains(&serde_json::json!("property-sorting")));
}

#[test]
fn build_ai_report_carries_the_given_tool_version_and_generated_at() {
    let config = papyrus_lints::Config::default();

    let report = build_ai_report(
        &config,
        "1.2.3",
        Vec::new(),
        0,
        "2024-02-29T23:59:59.042Z".to_string(),
    );

    assert_eq!(report.schema, AI_EXPORT_SCHEMA_URL);
    assert_eq!(report.header.tool, TOOL_NAME);
    assert_eq!(report.header.version, "1.2.3");
    assert_eq!(report.header.website, WEBSITE_URL);
    assert_eq!(report.header.target_game, TARGET_GAME);
    assert_eq!(report.header.generated_at, "2024-02-29T23:59:59.042Z");
    assert_eq!(report.findings.total_diagnostics, 0);
    assert!(report.rule_details.is_empty());
}

#[test]
fn build_ai_report_derives_rule_details_only_for_rules_that_actually_triggered() {
    let config = papyrus_lints::Config::default();
    let files = vec![AiFileReport {
        path: "Example.psc".to_string(),
        severity_counts: severity_counts(&[diagnostic("trailing-whitespace", "warning")]),
        rule_counts: rule_counts(&[diagnostic("trailing-whitespace", "warning")]),
        diagnostics: vec![diagnostic("trailing-whitespace", "warning")],
        parser_errors: Vec::new(),
        source: Some(AiSource::Content {
            content: "ScriptName Example\n".to_string(),
        }),
    }];

    let report = build_ai_report(&config, "1.2.3", files, 1, generated_at());

    assert_eq!(report.rule_details.len(), 1);
    assert_eq!(report.rule_details[0].rule, "trailing-whitespace");
    assert!(report.rule_details[0].auto_fixable);
    assert_eq!(report.findings.rule_counts["trailing-whitespace"], 1);
    assert_eq!(report.findings.severity_counts.warnings, 1);
}

#[test]
fn ai_source_serializes_content_hash_and_error_variants_with_a_type_tag() {
    let content = serde_json::to_value(AiSource::Content {
        content: "text".to_string(),
    })
    .unwrap();
    assert_eq!(content["type"], "content");
    assert_eq!(content["content"], "text");

    let hash = serde_json::to_value(AiSource::Hash {
        algorithm: "md5".to_string(),
        hash: "deadbeef".to_string(),
    })
    .unwrap();
    assert_eq!(hash["type"], "hash");
    assert_eq!(hash["algorithm"], "md5");

    let error = serde_json::to_value(AiSource::Error {
        message: "file moved".to_string(),
    })
    .unwrap();
    assert_eq!(error["type"], "error");
    assert_eq!(error["message"], "file moved");
}

#[test]
fn ai_source_round_trips_through_json_for_the_ipc_boundary() {
    let source = AiSource::Hash {
        algorithm: "md5".to_string(),
        hash: "deadbeef".to_string(),
    };
    let json = serde_json::to_string(&source).unwrap();
    let parsed: AiSource = serde_json::from_str(&json).unwrap();

    match parsed {
        AiSource::Hash { algorithm, hash } => {
            assert_eq!(algorithm, "md5");
            assert_eq!(hash, "deadbeef");
        }
        _ => panic!("expected the Hash variant to round-trip"),
    }
}

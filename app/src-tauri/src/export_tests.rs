use super::*;

fn finding(line: usize, column: usize, rule: &str, message: &str) -> OwnedDiagnostic {
    OwnedDiagnostic {
        line,
        column,
        rule: rule.to_string(),
        message: message.to_string(),
    }
}

#[test]
fn format_issues_as_text_renders_one_cli_style_line_per_finding() {
    let text = format_issues_as_text(vec![IssuesFileInput {
        path: "scripts/source/A.psc".to_string(),
        findings: vec![
            finding(1, 1, "trailing-whitespace", "[warning] trailing whitespace"),
            finding(
                5,
                3,
                "forbidden-functions",
                "[error] forbidden function used",
            ),
        ],
    }]);

    assert_eq!(
        text,
        "scripts/source/A.psc:1:1: [trailing-whitespace] [warning] trailing whitespace (https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace)\n\
         scripts/source/A.psc:5:3: [forbidden-functions] [error] forbidden function used (https://papyrus-lint.idrinth.de/rules.html#rule-forbidden-functions)"
    );
}

#[test]
fn format_issues_as_text_returns_an_empty_string_for_no_files() {
    assert_eq!(format_issues_as_text(vec![]), "");
}

#[test]
fn format_issues_as_json_mirrors_the_cli_json_shape_restricted_to_the_given_files() {
    let json = format_issues_as_json(vec![
        IssuesFileInput {
            path: "A.psc".to_string(),
            findings: vec![finding(
                1,
                1,
                "trailing-whitespace",
                "[warning] trailing whitespace",
            )],
        },
        IssuesFileInput {
            path: "B.psc".to_string(),
            findings: vec![finding(
                5,
                3,
                "forbidden-functions",
                "[error] forbidden function used",
            )],
        },
    ]);

    let report: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(report["files_with_diagnostics"], 2);
    assert_eq!(report["total_diagnostics"], 2);
    assert_eq!(report["files"][0]["path"], "A.psc");
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
    assert_eq!(report["files"][0]["diagnostics"][0]["level"], "warning");
    assert!(report["files"][0]["diff"].is_null());
    assert_eq!(report["files"][0]["parser_errors"], serde_json::json!([]));
    // Doesn't carry the CLI-only fields, which mean nothing for a filtered
    // export.
    assert!(report.get("scripts_checked").is_none());
    assert!(report.get("dry_run").is_none());
    assert!(report.get("success").is_none());
}

#[test]
fn format_issues_as_json_returns_an_empty_report_for_no_files() {
    let json = format_issues_as_json(vec![]);
    let report: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

    assert_eq!(report["files"], serde_json::json!([]));
    assert_eq!(report["files_with_diagnostics"], 0);
    assert_eq!(report["total_diagnostics"], 0);
}

#[test]
fn format_issues_for_ai_base_builds_the_shared_header_configuration_and_findings() {
    let json = format_issues_for_ai_base(
        vec![AiIssuesFileInput {
            path: "Example.psc".to_string(),
            findings: vec![finding(
                1,
                1,
                "trailing-whitespace",
                "[warning] trailing whitespace",
            )],
            source: Some(AiSource::Content {
                content: "ScriptName Example   \n".to_string(),
            }),
        }],
        papyrus_lints::Config::default(),
        "1.2.3".to_string(),
    );

    let report: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(report["header"]["tool"], "Papyrus Lint");
    assert_eq!(report["header"]["version"], "1.2.3");
    assert_eq!(report["findings"]["total_diagnostics"], 1);
    assert_eq!(report["findings"]["files"][0]["path"], "Example.psc");
    // The severity prefix is stripped from `message`, unlike the CLI's own
    // `--format ai` (which keeps it), since `level` already carries it.
    assert_eq!(
        report["findings"]["files"][0]["diagnostics"][0]["message"],
        "trailing whitespace"
    );
    assert_eq!(
        report["findings"]["files"][0]["diagnostics"][0]["level"],
        "warning"
    );
    assert_eq!(report["findings"]["files"][0]["source"]["type"], "content");
    assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
    assert!(report["configuration"]["enabled_rules"].is_array());
    assert!(report["configuration"].get("rules").is_none());
}

#[test]
fn format_issues_for_ai_base_leaves_source_null_when_none_was_attached() {
    let json = format_issues_for_ai_base(
        vec![AiIssuesFileInput {
            path: "Example.psc".to_string(),
            findings: vec![finding(
                1,
                1,
                "trailing-whitespace",
                "[warning] trailing whitespace",
            )],
            source: None,
        }],
        papyrus_lints::Config::default(),
        "1.2.3".to_string(),
    );

    let report: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(report["findings"]["files"][0]["source"].is_null());
}

#[test]
fn format_issues_as_text_preserves_file_order_and_skips_empty_findings() {
    let text = format_issues_as_text(vec![
        IssuesFileInput {
            path: "Empty.psc".to_string(),
            findings: Vec::new(),
        },
        IssuesFileInput {
            path: "B.psc".to_string(),
            findings: vec![finding(2, 4, "trailing-whitespace", "trailing whitespace")],
        },
        IssuesFileInput {
            path: "A.psc".to_string(),
            findings: vec![finding(1, 2, "identifier-casing", "identifier casing")],
        },
    ]);

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("B.psc:2:4:"));
    assert!(lines[1].starts_with("A.psc:1:2:"));
}

#[test]
fn format_issues_for_ai_base_aggregates_counts_across_files() {
    let json = format_issues_for_ai_base(
        vec![
            AiIssuesFileInput {
                path: "A.psc".to_string(),
                findings: vec![finding(
                    1,
                    1,
                    "trailing-whitespace",
                    "[warning] trailing whitespace",
                )],
                source: Some(AiSource::Hash {
                    algorithm: "md5".to_string(),
                    hash: "0123456789abcdef0123456789abcdef".to_string(),
                }),
            },
            AiIssuesFileInput {
                path: "B.psc".to_string(),
                findings: vec![finding(
                    3,
                    2,
                    "forbidden-functions",
                    "[error] forbidden function used",
                )],
                source: Some(AiSource::Error {
                    message: "could not read source".to_string(),
                }),
            },
        ],
        papyrus_lints::Config::default(),
        "1.2.3".to_string(),
    );

    let report: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(report["findings"]["total_diagnostics"], 2);
    assert_eq!(report["findings"]["severity_counts"]["warnings"], 1);
    assert_eq!(report["findings"]["severity_counts"]["errors"], 1);
    assert_eq!(report["findings"]["files"][0]["source"]["type"], "hash");
    assert_eq!(report["findings"]["files"][1]["source"]["type"], "error");
}

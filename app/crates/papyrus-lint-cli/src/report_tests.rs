use super::*;

fn outcome(
    plain_text: &str,
    parse_failed: bool,
    should_fail: bool,
    diagnostic_count: usize,
    fixed: bool,
) -> Result<FileOutcome, String> {
    Ok(FileOutcome {
        plain_text: plain_text.as_bytes().to_vec(),
        json_file: None,
        ai_file: None,
        parse_failed,
        should_fail,
        has_diagnostics: diagnostic_count > 0,
        diagnostic_count,
        fixed,
    })
}

fn flush_plain(
    outcomes: Vec<Result<FileOutcome, String>>,
    scripts_checked: usize,
    fix: bool,
    dry_run: bool,
    use_color: bool,
    progress: bool,
) -> (u8, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = fold_and_flush_report(
        outcomes,
        OutputFormat::Plain,
        &papyrus_lints::Config::default(),
        scripts_checked,
        fix,
        dry_run,
        use_color,
        progress,
        None,
        &mut stdout,
        &mut stderr,
    );
    (
        status,
        String::from_utf8(stdout).expect("plain report should be UTF-8"),
        String::from_utf8(stderr).expect("error output should be UTF-8"),
    )
}

#[test]
fn plain_report_preserves_file_order_and_summarizes_diagnostics() {
    let (status, stdout, stderr) = flush_plain(
        vec![
            outcome("first\n", false, false, 2, false),
            outcome("second\n", false, true, 1, false),
        ],
        3,
        false,
        false,
        false,
        false,
    );

    assert_eq!(status, 1);
    assert_eq!(stderr, "");
    assert_eq!(
        stdout,
        "first\nsecond\nPapyrusLinterCLI: 3 problem(s) found in 2 of 3 script(s).\n"
    );
}

#[test]
fn plain_report_distinguishes_parse_failures_from_clean_runs() {
    let (status, stdout, _) = flush_plain(
        vec![outcome("", true, false, 0, false)],
        1,
        false,
        false,
        false,
        false,
    );
    assert_eq!(status, 1);
    assert_eq!(
        stdout,
        "PapyrusLinterCLI: parser/lexer error(s) found in 1 script(s).\n"
    );

    let (status, stdout, _) = flush_plain(Vec::new(), 0, false, false, true, false);
    assert_eq!(status, 0);
    assert_eq!(
        stdout,
        "\x1b[32mPapyrusLinterCLI: no problems found in 0 script(s).\x1b[0m\n"
    );
}

#[test]
fn plain_report_describes_applied_and_previewed_fixes() {
    let (_, stdout, _) = flush_plain(
        vec![outcome("", false, false, 0, true)],
        1,
        true,
        false,
        false,
        false,
    );
    assert!(stdout.ends_with(" (1 script(s) fixed.)\n"));

    let (_, stdout, _) = flush_plain(
        vec![outcome("", false, false, 0, true)],
        1,
        true,
        true,
        false,
        false,
    );
    assert!(stdout.ends_with(" (1 script(s) would be fixed.)\n"));
}

#[test]
fn progress_report_separates_progress_from_the_summary() {
    let (_, stdout, _) = flush_plain(Vec::new(), 0, false, false, false, true);
    assert!(stdout.starts_with('\n'));
    assert!(stdout.ends_with("no problems found in 0 script(s).\n"));
}

#[test]
fn json_report_aggregates_counts_and_fix_metadata() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = fold_and_flush_report(
        vec![outcome("", false, false, 2, true)],
        OutputFormat::Json,
        &papyrus_lints::Config::default(),
        4,
        true,
        true,
        false,
        false,
        None,
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0);
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&stdout).expect("report should contain JSON");
    assert_eq!(report["scripts_checked"], 4);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 2);
    assert_eq!(report["files_fixed"], 1);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["success"], true);
}

#[test]
fn report_write_failure_returns_io_status_without_using_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temporary directory");
    let missing_parent = dir.path().join("missing/report.txt");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = fold_and_flush_report(
        Vec::new(),
        OutputFormat::Plain,
        &papyrus_lints::Config::default(),
        0,
        false,
        false,
        false,
        false,
        Some(&missing_parent),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 2);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .expect("error output should be UTF-8")
        .contains("error: failed to write"));
}

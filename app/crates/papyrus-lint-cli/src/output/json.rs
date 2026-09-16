use serde::Serialize;

/// A single diagnostic as printed by `--json`, mirroring the plain-text
/// `<path>:<line>:<column>: [<rule>] <message>` line but with `level`
/// (see [`papyrus_lints::Diagnostic::level`]) broken out as its own field
/// rather than left for a consumer to parse back out of `message`.
#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub level: &'static str,
    pub message: String,
    /// This rule's own documentation link (`RuleTags::doc_url`), so a
    /// consumer (an editor extension, the SublimeLinter plugin) can jump a
    /// user straight to it instead of just showing the rule id. `None` for
    /// a rule with no [`papyrus_lints::tags`] metadata (e.g. a
    /// compiler-reported diagnostic — see
    /// `papyrus_lint_core::compile_diagnostics`).
    pub doc_url: Option<String>,
}

/// Looks up `rule`'s [`papyrus_lints::tags::RuleTags::doc_url`], for
/// building a [`JsonDiagnostic`]/`AiRuleDetails`.
pub(crate) fn doc_url_for(rule: &str) -> Option<String> {
    papyrus_lints::tags::tags_for(rule).map(|tags| tags.doc_url())
}

/// Converts a script's (or the `--blob` source's) already-finalized
/// diagnostics into their `--json`/`--format ai` shape. Shared by
/// [`crate::run`] and [`crate::run_blob`] so the two never disagree on how a
/// [`papyrus_lints::Diagnostic`] maps onto the reported JSON fields.
pub(crate) fn to_json_diagnostics(
    diagnostics: &[papyrus_lints::Diagnostic],
) -> Vec<JsonDiagnostic> {
    diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            line: d.line,
            column: d.column,
            rule: d.rule,
            level: d.level(),
            message: d.message.clone(),
            doc_url: doc_url_for(d.rule),
        })
        .collect()
}

/// One resolved script's diagnostics, as printed by `--json`. Every
/// resolved script gets an entry, even one with no diagnostics, so a
/// consumer (e.g. an editor plugin) can clear stale diagnostics for a
/// file that's since become clean.
#[derive(Debug, Serialize)]
pub struct JsonFileReport {
    pub path: String,
    pub diagnostics: Vec<JsonDiagnostic>,
    /// The standard unified diff between this script's original source and
    /// what `fix` would have written, only non-`null` when run with `fix
    /// --dry-run` and this script would actually have changed.
    pub diff: Option<String>,
}

/// The full report printed to stdout by `--json`, in place of the
/// plain-text diagnostics lines and summary.
#[derive(Debug, Serialize)]
pub struct JsonReport {
    pub files: Vec<JsonFileReport>,
    pub scripts_checked: usize,
    pub files_with_diagnostics: usize,
    pub total_diagnostics: usize,
    /// Only present when run with the `fix` subcommand. Under `--dry-run`,
    /// counts scripts that *would* have been fixed rather than scripts
    /// actually rewritten on disk.
    pub files_fixed: Option<usize>,
    /// Whether this run was `fix --dry-run`: no file was written, and each
    /// changed script's [`JsonFileReport::diff`] instead shows what would
    /// have changed. Always `false` outside `fix --dry-run`.
    pub dry_run: bool,
    /// Whether the run would exit `0`: no diagnostics counted as a
    /// failure per `fail_on_warning`/`fail_on_info` (see
    /// [`papyrus_lints::Config::should_fail_on`]).
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use std::fs;

    #[test]
    fn json_flag_prints_a_single_json_report_instead_of_plain_text() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert_eq!(stderr, "");
        assert!(!stdout.contains("PapyrusLinterCLI:"));

        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be a single JSON document");
        assert_eq!(report["success"], true);
        assert_eq!(report["scripts_checked"], 1);
        assert_eq!(report["files_with_diagnostics"], 1);
        assert_eq!(report["total_diagnostics"], 1);
        assert!(report["files_fixed"].is_null());
        let files = report["files"]
            .as_array()
            .expect("files should be an array");
        assert_eq!(files.len(), 1);
        let diagnostics = files[0]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["rule"], "trailing-whitespace");
        assert_eq!(diagnostics[0]["level"], "warning");
        assert_eq!(diagnostics[0]["line"], 1);
    }

    #[test]
    fn json_flag_lists_every_resolved_script_including_clean_ones() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["success"], true);
        let files = report["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_flag_combines_with_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        let files = report["files"].as_array().unwrap();
        let diagnostics = files[0]["diagnostics"].as_array().unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("Game.GetPlayer")));
    }

    #[test]
    fn json_flag_combines_with_fix_dry_run() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
            "--dry-run must never write to the file, even combined with --json"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["dry_run"], true);
        assert_eq!(report["files_fixed"], 1);
        let files = report["files"].as_array().unwrap();
        let diff = files[0]["diff"].as_str().expect("diff should be a string");
        assert!(diff.contains("-ScriptName Example   \n"));
        assert!(diff.contains("+ScriptName Example\n"));
    }

    #[test]
    fn json_flag_can_precede_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "fix".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        assert_eq!(report["total_diagnostics"], 0);
        assert_eq!(report["success"], true);
    }

    #[test]
    fn json_output_is_never_colorized_even_when_color_is_always() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured_with_terminal_stdout(
            &[
                "--json".to_string(),
                "--color=always".to_string(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(!stdout.contains('\x1b'));
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("colored JSON would not parse");
        assert_eq!(report["total_diagnostics"], 1);
    }
}

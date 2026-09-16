pub(crate) const ANSI_RESET: &str = "\x1b[0m";
pub(crate) const ANSI_BOLD: &str = "\x1b[1m";
pub(crate) const ANSI_DIM: &str = "\x1b[2m";
pub(crate) const ANSI_RED: &str = "\x1b[31m";
pub(crate) const ANSI_YELLOW: &str = "\x1b[33m";
pub(crate) const ANSI_CYAN: &str = "\x1b[36m";
pub(crate) const ANSI_GREEN: &str = "\x1b[32m";

/// Whether/when to colorize the plain-text report, set by the `--color`
/// flag (see [`USAGE`]). Never affects `--json` output, which is meant for
/// tooling rather than a terminal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Wraps `text` in `code`/reset ANSI escapes when `use_color` is true,
/// otherwise returns it unchanged.
pub(crate) fn colorize(text: &str, code: &str, use_color: bool) -> String {
    if use_color {
        format!("{code}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

pub(crate) fn level_color(level: &str) -> &'static str {
    match level {
        "error" => ANSI_RED,
        "warning" => ANSI_YELLOW,
        "info" => ANSI_CYAN,
        _ => ANSI_RESET,
    }
}

/// Renders one diagnostic's plain-text report line (`<path>:<line>:<column>:
/// [<rule>] <message>`), colorizing the location, the rule tag, and the
/// `[error]`/`[warning]`/`[info]` level tag already embedded at the front of
/// `diagnostic.message` (see [`papyrus_lints::Diagnostic::level`]) when
/// `use_color` is true. A rule with known [`papyrus_lints::tags`] metadata
/// (i.e. a real lint rather than e.g. a compiler-reported diagnostic) gets
/// its documentation link (`RuleTags::doc_url`) appended, so a reader can
/// jump straight to that rule's own explanation instead of just seeing its
/// id.
pub(crate) fn format_diagnostic_line(
    path_display: &str,
    diagnostic: &papyrus_lints::Diagnostic,
    use_color: bool,
) -> String {
    let doc_url_suffix = papyrus_lints::tags::tags_for(diagnostic.rule)
        .map(|tags| format!(" ({})", tags.doc_url()))
        .unwrap_or_default();

    if !use_color {
        return format!(
            "{}:{}:{}: [{}] {}{}",
            path_display,
            diagnostic.line,
            diagnostic.column,
            diagnostic.rule,
            diagnostic.message,
            doc_url_suffix
        );
    }

    let level = diagnostic.level();
    let level_tag = format!("[{level}]");
    let message = match diagnostic.message.strip_prefix(level_tag.as_str()) {
        Some(rest) => format!("{}{rest}", colorize(&level_tag, level_color(level), true)),
        None => diagnostic.message.clone(),
    };

    format!(
        "{}: {} {}{}",
        colorize(
            &format!("{path_display}:{}:{}", diagnostic.line, diagnostic.column),
            ANSI_BOLD,
            true
        ),
        colorize(&format!("[{}]", diagnostic.rule), ANSI_DIM, true),
        message,
        if doc_url_suffix.is_empty() {
            String::new()
        } else {
            colorize(&doc_url_suffix, ANSI_DIM, true)
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use std::fs;

    #[test]
    fn plain_text_report_is_uncolored_when_stdout_is_not_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) =
            run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], false);

        assert_eq!(code, 0);
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    fn color_auto_colorizes_when_stdout_is_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) =
            run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], true);

        assert_eq!(code, 0);
        assert!(stdout.contains('\x1b'));
        // The rule id and level tag both still appear verbatim inside the
        // colorized escapes, so consumers scraping for them (and the other
        // tests here) still find them.
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("[warning]"));
    }

    #[test]
    fn color_never_disables_color_even_on_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured_with_terminal_stdout(
            &[
                "--color".to_string(),
                "never".to_string(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    fn color_always_enables_color_even_without_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured(&[
            "--color=always".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.contains('\x1b'));
    }

    #[test]
    fn color_auto_does_not_colorize_a_file_written_via_output() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");
        let output_path = dir.path().join("report.txt");

        let (code, _stdout, _stderr) = run_captured_with_terminal_stdout(
            &[
                "--output".to_string(),
                output_path.to_string_lossy().into_owned(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(!contents.contains('\x1b'));
    }

    #[test]
    fn color_flag_rejects_an_unknown_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--color".to_string(),
            "rainbow".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--color must be"));
    }

    #[test]
    fn color_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--color".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn diagnostic_formatter_colorizes_each_structural_part() {
        let diagnostic = papyrus_lints::Diagnostic {
            line: 4,
            column: 7,
            rule: "example-rule",
            message: "[warning] example message".to_string(),
        };

        let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

        assert!(formatted.contains("\x1b[1mExample.psc:4:7\x1b[0m"));
        assert!(formatted.contains("\x1b[2m[example-rule]\x1b[0m"));
        assert!(formatted.contains("\x1b[33m[warning]\x1b[0m example message"));
    }

    #[test]
    fn diagnostic_formatter_preserves_an_untagged_message() {
        let diagnostic = papyrus_lints::Diagnostic {
            line: 1,
            column: 2,
            rule: "example-rule",
            message: "example message without a level tag".to_string(),
        };

        let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

        assert!(formatted.ends_with("example message without a level tag"));
        assert!(!formatted.contains("\x1b[31m[error]"));
    }

    #[test]
    fn level_colors_cover_info_and_unknown_diagnostic_levels() {
        assert_eq!(level_color("info"), ANSI_CYAN);
        assert_eq!(level_color("notice"), ANSI_RESET);

        let info = papyrus_lints::Diagnostic {
            line: 2,
            column: 3,
            rule: "example-rule",
            message: "[info] informational diagnostic".to_string(),
        };
        let formatted = format_diagnostic_line("Example.psc", &info, true);

        assert!(formatted.contains(&format!("{ANSI_CYAN}[info]{ANSI_RESET}")));
        assert!(formatted.contains(" informational diagnostic"));
    }
}

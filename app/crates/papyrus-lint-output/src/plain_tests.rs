use super::*;
use crate::diagnostic::OwnedDiagnostic;

fn diagnostic(rule: &str, message: &str) -> OwnedDiagnostic {
    OwnedDiagnostic {
        line: 4,
        column: 7,
        rule: rule.to_string(),
        message: message.to_string(),
    }
}

#[test]
fn resolve_color_always_and_never_ignore_every_other_input() {
    assert!(resolve_color(ColorChoice::Always, None, false));
    assert!(!resolve_color(ColorChoice::Never, None, true));
}

#[test]
fn resolve_color_auto_requires_a_terminal_and_no_output_path() {
    let output = Some(std::path::Path::new("report.txt"));
    assert!(!resolve_color(ColorChoice::Auto, output, true));

    if anstyle_query::clicolor_force() {
        assert!(resolve_color(ColorChoice::Auto, None, false));
        assert!(resolve_color(ColorChoice::Auto, None, true));
        return;
    }

    assert!(!resolve_color(ColorChoice::Auto, None, false));

    let env_blocks_auto = anstyle_query::no_color() || anstyle_query::clicolor() == Some(false);
    assert_eq!(
        resolve_color(ColorChoice::Auto, None, true),
        !env_blocks_auto
    );
}

#[test]
fn colorize_wraps_text_only_when_use_color_is_true() {
    assert_eq!(colorize("hi", ANSI_RED, true), "\x1b[31mhi\x1b[0m");
    assert_eq!(colorize("hi", ANSI_RED, false), "hi");
}

#[test]
fn level_colors_cover_every_known_and_an_unknown_level() {
    assert_eq!(level_color("error"), ANSI_RED);
    assert_eq!(level_color("warning"), ANSI_YELLOW);
    assert_eq!(level_color("info"), ANSI_CYAN);
    assert_eq!(level_color("notice"), ANSI_RESET);
}

#[test]
fn diagnostic_formatter_renders_the_cli_style_line_uncolored() {
    let finding = diagnostic("example-rule", "[warning] example message");

    let formatted = format_diagnostic_line("Example.psc", &finding, false);

    assert_eq!(
        formatted,
        "Example.psc:4:7: [example-rule] [warning] example message"
    );
}

#[test]
fn diagnostic_formatter_appends_a_known_rules_doc_url() {
    let finding = diagnostic("trailing-whitespace", "[warning] trailing whitespace");

    let formatted = format_diagnostic_line("Example.psc", &finding, false);

    assert!(formatted
        .ends_with("(https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace)"));
}

#[test]
fn diagnostic_formatter_colorizes_each_structural_part() {
    let finding = diagnostic("example-rule", "[warning] example message");

    let formatted = format_diagnostic_line("Example.psc", &finding, true);

    assert!(formatted.contains("\x1b[1mExample.psc:4:7\x1b[0m"));
    assert!(formatted.contains("\x1b[2m[example-rule]\x1b[0m"));
    assert!(formatted.contains("\x1b[33m[warning]\x1b[0m example message"));
}

#[test]
fn diagnostic_formatter_preserves_an_untagged_message() {
    let finding = diagnostic("example-rule", "example message without a level tag");

    let formatted = format_diagnostic_line("Example.psc", &finding, true);

    assert!(formatted.ends_with("example message without a level tag"));
    assert!(!formatted.contains("\x1b[31m[error]"));
}

#[test]
fn diagnostic_formatter_works_directly_against_native_papyrus_lints_diagnostics() {
    let finding = papyrus_lints::Diagnostic {
        line: 1,
        column: 2,
        rule: "example-rule",
        message: "[info] informational diagnostic".to_string(),
    };

    let formatted = format_diagnostic_line("Example.psc", &finding, true);

    assert!(formatted.contains(&format!("{ANSI_CYAN}[info]{ANSI_RESET}")));
    assert!(formatted.contains(" informational diagnostic"));
}

#[test]
fn parser_error_formatter_renders_lex_and_parse_lines_uncolored() {
    let parse_err = crate::JsonParserError {
        kind: crate::ParserErrorKind::Parse,
        line: 2,
        column: 18,
        message: "expected ')'".to_string(),
    };
    assert_eq!(
        format_parser_error_line("Broken.psc", &parse_err, false),
        "Broken.psc:2:18: [parse] expected ')'"
    );

    let lex_err = crate::JsonParserError {
        kind: crate::ParserErrorKind::Lex,
        line: 1,
        column: 3,
        message: "unexpected character".to_string(),
    };
    assert_eq!(
        format_parser_error_line("Broken.psc", &lex_err, false),
        "Broken.psc:1:3: [lex] unexpected character"
    );
}

#[test]
fn parser_error_formatter_colorizes_location_and_kind() {
    let error = crate::JsonParserError {
        kind: crate::ParserErrorKind::Parse,
        line: 2,
        column: 18,
        message: "expected ')'".to_string(),
    };
    let formatted = format_parser_error_line("Broken.psc", &error, true);
    assert!(formatted.contains(ANSI_BOLD));
    assert!(formatted.contains(ANSI_DIM));
    assert!(formatted.contains("expected ')'"));
    assert!(formatted.contains("[parse]"));
}

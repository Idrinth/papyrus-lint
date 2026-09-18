use crate::diagnostic::DiagnosticLike;

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_DIM: &str = "\x1b[2m";
pub const ANSI_RED: &str = "\x1b[31m";
pub const ANSI_YELLOW: &str = "\x1b[33m";
pub const ANSI_CYAN: &str = "\x1b[36m";
pub const ANSI_GREEN: &str = "\x1b[32m";

/// Whether/when to colorize the plain-text report, set by the CLI's
/// `--color` flag (see its own `USAGE`). Never affects JSON/AI output,
/// which is meant for tooling rather than a terminal; the desktop app's own
/// plain-text export never colorizes at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Resolves whether the plain-text report should actually be colorized,
/// given `--color <when>`, whether the report is being redirected to a file
/// via `--output` (never a terminal), and whether stdout itself is one.
///
/// `--color always` / `--color never` win outright. `--color auto` follows
/// the [NO_COLOR](https://no-color.org/) and
/// [CLICOLOR](https://bixense.com/clicolors/) conventions, implemented by
/// [`anstyle_query`] (the same lookups clap/anstream use) rather than a
/// hand-rolled `NO_COLOR` check:
///
/// - a non-empty `CLICOLOR_FORCE` enables color even when stdout is not a TTY
/// - a non-empty `NO_COLOR`, or `CLICOLOR=0`, disables auto color
/// - otherwise color only when stdout is a terminal and `--output` is not used
pub fn resolve_color(
    color_choice: ColorChoice,
    output_path: Option<&std::path::Path>,
    stdout_is_terminal: bool,
) -> bool {
    match color_choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => auto_color(output_path.is_none(), stdout_is_terminal),
    }
}

fn auto_color(report_goes_to_stdout: bool, stdout_is_terminal: bool) -> bool {
    if !report_goes_to_stdout {
        return false;
    }
    if anstyle_query::clicolor_force() {
        return true;
    }
    if anstyle_query::no_color() || anstyle_query::clicolor() == Some(false) {
        return false;
    }
    stdout_is_terminal
}

/// Wraps `text` in `code`/reset ANSI escapes when `use_color` is true,
/// otherwise returns it unchanged.
pub fn colorize(text: &str, code: &str, use_color: bool) -> String {
    if use_color {
        format!("{code}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

pub fn level_color(level: &str) -> &'static str {
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
/// the message when `use_color` is true. A rule with known
/// [`papyrus_lints::tags`] metadata (i.e. a real lint rather than e.g. a
/// compiler-reported diagnostic) gets its documentation link
/// (`RuleTags::doc_url`) appended, so a reader can jump straight to that
/// rule's own explanation instead of just seeing its id.
pub fn format_diagnostic_line<D: DiagnosticLike>(
    path_display: &str,
    diagnostic: &D,
    use_color: bool,
) -> String {
    let doc_url_suffix = crate::doc_url_for(diagnostic.rule())
        .map(|url| format!(" ({url})"))
        .unwrap_or_default();

    if !use_color {
        return format!(
            "{}:{}:{}: [{}] {}{}",
            path_display,
            diagnostic.line(),
            diagnostic.column(),
            diagnostic.rule(),
            diagnostic.message(),
            doc_url_suffix
        );
    }

    let level = crate::level_of(diagnostic.message());
    let level_tag = format!("[{level}]");
    let message = match diagnostic.message().strip_prefix(level_tag.as_str()) {
        Some(rest) => format!("{}{rest}", colorize(&level_tag, level_color(level), true)),
        None => diagnostic.message().to_string(),
    };

    format!(
        "{}: {} {}{}",
        colorize(
            &format!(
                "{path_display}:{}:{}",
                diagnostic.line(),
                diagnostic.column()
            ),
            ANSI_BOLD,
            true
        ),
        colorize(&format!("[{}]", diagnostic.rule()), ANSI_DIM, true),
        message,
        if doc_url_suffix.is_empty() {
            String::new()
        } else {
            colorize(&doc_url_suffix, ANSI_DIM, true)
        }
    )
}

#[cfg(test)]
#[path = "plain_tests.rs"]
mod tests;

//! Turns PapyrusCompiler.exe's own reported errors into
//! [`papyrus_lints::Diagnostic`]s, so a syntax mistake the compiler itself
//! rejects (but the lint engine's own, more forgiving parser doesn't) still
//! shows up in the same diagnostics list — see [`crate::compiler::check_psc_file`].
//!
//! Every line PapyrusCompiler.exe reports a problem on follows the same
//! shape:
//!
//! ```text
//! <path>(<line>,<column>): <message>
//! ```
//!
//! e.g. `MyScript.psc(12,4): no viable alternative at character ';'`, or
//! `<unknown>(0,0): unable to locate script ...` for a failure with no
//! specific source location. Everything else the compiler prints (its
//! "Starting N compile threads...", "Compiling ...", batch summary, and
//! "Failed on ..." lines) carries no such `(line,column):` marker and is
//! ignored.

use papyrus_lints::Diagnostic;

use crate::compiler::CompileOutcome;

/// This diagnostic's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "compiler-error";

/// Parses every `(<line>,<column>): <message>` location marker out of
/// `outcome`'s stdout and stderr (PapyrusCompiler.exe writes errors to
/// either, depending on the failure) into [`Diagnostic`]s, tagged
/// `[error]` since a script the compiler itself rejects can never work in
/// game regardless of what level any other lint would have assigned it.
/// Returns an empty `Vec` for a successful compile, since a source with no
/// reported errors has no markers to find in the first place.
///
/// A `0` line or column (e.g. `<unknown>(0,0): ...`, reported when the
/// compiler couldn't even locate the script to compile) is clamped to `1`,
/// matching [`Diagnostic`]'s documented 1-indexed convention.
pub fn parse_compile_errors(outcome: &CompileOutcome) -> Vec<Diagnostic> {
    outcome
        .stdout
        .lines()
        .chain(outcome.stderr.lines())
        .filter_map(parse_line)
        .collect()
}

/// Parses the last (rightmost) valid `(<line>,<column>): <message>` marker
/// out of a single line of compiler output, if any. Scanning for the
/// *last* match (rather than the first) matters because the path
/// preceding the marker can itself contain a parenthesized segment (e.g.
/// `C:\Program Files (x86)\...`) — one that happens to look like a marker
/// is rejected below since what follows it isn't `<digits>,<digits>):`,
/// but scanning past every `(` this way, rather than stopping at the
/// first one, is what lets the real marker (always the last `(...)`
/// before the message) still be found.
fn parse_line(line: &str) -> Option<Diagnostic> {
    let mut found = None;
    let mut search_from = 0;
    while let Some(open) = line[search_from..].find('(') {
        let open = search_from + open;
        if let Some((line_no, column_no, message)) = parse_marker(&line[open + 1..]) {
            found = Some((line_no, column_no, message));
        }
        search_from = open + 1;
    }

    let (line_no, column_no, message) = found?;
    Some(Diagnostic {
        line: line_no.max(1),
        column: column_no.max(1),
        message: format!("[error] {message}"),
        rule: RULE,
    })
}

/// Parses a `<line>,<column>): <message>` marker body (everything after
/// the opening `(` a caller has already found) into its three parts.
fn parse_marker(after_open: &str) -> Option<(usize, usize, &str)> {
    let (line_part, after_line) = after_open.split_once(',')?;
    let line_no: usize = line_part.trim().parse().ok()?;

    let (column_part, after_column) = after_line.split_once(')')?;
    let column_no: usize = column_part.trim().parse().ok()?;

    let message = after_column.strip_prefix(':')?.trim();
    Some((line_no, column_no, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(stdout: &str) -> CompileOutcome {
        CompileOutcome {
            success: false,
            stdout: stdout.to_string(),
            stderr: String::new(),
            personal_data_stripped: false,
        }
    }

    #[test]
    fn successful_compile_yields_no_diagnostics() {
        let outcome = CompileOutcome {
            success: true,
            stdout: "Batch compile of 1 files finished. 1 succeeded, 0 failed.\n".to_string(),
            stderr: String::new(),
            personal_data_stripped: true,
        };

        assert!(parse_compile_errors(&outcome).is_empty());
    }

    #[test]
    fn parses_a_single_error_line() {
        let outcome = outcome("MyScript.psc(12,4): no viable alternative at character ';'\n");

        let diagnostics = parse_compile_errors(&outcome);

        assert_eq!(
            diagnostics,
            vec![Diagnostic {
                line: 12,
                column: 4,
                message: "[error] no viable alternative at character ';'".to_string(),
                rule: RULE,
            }]
        );
    }

    #[test]
    fn ignores_summary_lines_with_no_location_marker() {
        let outcome = outcome(
            "Starting 1 compile threads for 1 files...\n\
             Compiling \"C:\\Data\\SCRIPTS\\SOURCE\"...\n\
             No output generated for C:\\Data\\SCRIPTS\\SOURCE, compilation failed.\n\
             \n\
             Batch compile of 1 files finished. 0 succeeded, 1 failed.\n\
             Failed on C:\\Data\\SCRIPTS\\SOURCE\n",
        );

        assert!(parse_compile_errors(&outcome).is_empty());
    }

    #[test]
    fn is_not_confused_by_a_parenthesized_path_segment() {
        let outcome = outcome(
            "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Skyrim Special Edition\\Data\\scripts\\source\\Example.PSC(1,63): no viable alternative at character ';'\n",
        );

        let diagnostics = parse_compile_errors(&outcome);

        assert_eq!(
            diagnostics,
            vec![Diagnostic {
                line: 1,
                column: 63,
                message: "[error] no viable alternative at character ';'".to_string(),
                rule: RULE,
            }]
        );
    }

    #[test]
    fn clamps_a_zero_line_and_column_to_one() {
        let outcome =
            outcome("<unknown>(0,0): unable to locate script C:\\Data\\SCRIPTS\\SOURCE\n");

        let diagnostics = parse_compile_errors(&outcome);

        assert_eq!(
            diagnostics,
            vec![Diagnostic {
                line: 1,
                column: 1,
                message: "[error] unable to locate script C:\\Data\\SCRIPTS\\SOURCE".to_string(),
                rule: RULE,
            }]
        );
    }

    #[test]
    fn parses_every_error_line_in_a_full_compiler_transcript() {
        let outcome = outcome(
            "Starting 1 compile threads for 1 files...\n\
             Compiling \"C:\\Data\\SCRIPTS\\SOURCE\"...\n\
             No output generated for C:\\Data\\SCRIPTS\\SOURCE, compilation failed.\n\
             \n\
             Batch compile of 1 files finished. 0 succeeded, 1 failed.\n\
             Failed on C:\\Data\\SCRIPTS\\SOURCE\n\
             \n\
             C:\\Data\\scripts\\source\\Example.PSC(1,63): no viable alternative at character ';'\n\
             C:\\Data\\scripts\\source\\Example.PSC(3,65): no viable alternative at character ';'\n\
             C:\\Data\\scripts\\source\\Example.PSC(1,0): missing EOF at 'Scriptname'\n\
             <unknown>(0,0): unable to locate script C:\\Data\\SCRIPTS\\SOURCE\n",
        );

        let diagnostics = parse_compile_errors(&outcome);

        assert_eq!(diagnostics.len(), 4);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 63);
        assert_eq!(diagnostics[1].line, 3);
        assert_eq!(diagnostics[1].column, 65);
        assert_eq!(diagnostics[2].line, 1);
        assert_eq!(diagnostics[2].column, 1);
        assert_eq!(diagnostics[3].line, 1);
        assert_eq!(diagnostics[3].column, 1);
        assert!(diagnostics.iter().all(|d| d.rule == RULE));
        assert!(diagnostics
            .iter()
            .all(|d| d.message.starts_with("[error] ")));
    }

    #[test]
    fn checks_both_stdout_and_stderr() {
        let outcome = CompileOutcome {
            success: false,
            stdout: "Example.psc(1,1): from stdout\n".to_string(),
            stderr: "Example.psc(2,2): from stderr\n".to_string(),
            personal_data_stripped: false,
        };

        let diagnostics = parse_compile_errors(&outcome);

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .any(|d| d.line == 1 && d.message == "[error] from stdout"));
        assert!(diagnostics
            .iter()
            .any(|d| d.line == 2 && d.message == "[error] from stderr"));
    }

    #[test]
    fn ignores_a_line_with_no_marker_at_all() {
        assert!(parse_line("just some unrelated text").is_none());
    }
}

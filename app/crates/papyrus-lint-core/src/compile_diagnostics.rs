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
    if outcome.success {
        return Vec::new();
    }

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
#[path = "compile_diagnostics_tests.rs"]
mod tests;

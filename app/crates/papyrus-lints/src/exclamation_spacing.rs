//! Requires exactly one space between a `!` negation operator and the
//! expression it negates, e.g. `! bReady` instead of `!bReady`, so the
//! negation is easier to spot at a glance.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "exclamation-spacing";

/// Checks for a `!` negation operator (never `!=`, which the lexer tokenizes
/// separately) whose following characters, on the same line, aren't exactly
/// one plain space. Always reported as a `[warning]`. A `!` on a line
/// protected by a CreationKit fragment-code wrapper (see [`fragment_code`])
/// is never flagged, and neither is a `!` with nothing but a line ending (or
/// end of file) after it — inserting a space there would just be trailing
/// whitespace, which the "Trailing whitespace" fix would strip right back
/// off, so there's nothing this lint can usefully require. A `!` directly
/// followed by another `!` with no space in between (a chained/double
/// negation like `!!bReady`) is left alone too: only the last `!` in such a
/// run needs the trailing space, since spreading the run's own `!`s apart
/// makes the idiom harder to read, not easier.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let protected = fragment_code::protected_lines(source);
    let tokens = match papyrus_parser::tokenize(source) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut diagnostics = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Not || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let (start, end) = whitespace_run(bytes, offset);
        if starts_another_negation(bytes, start, end) {
            continue;
        }
        if !at_end_of_line(bytes, end) && !is_single_space(bytes, start, end) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[warning] '!' must be followed by exactly one space".to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// Rewrites the whitespace immediately after every `!` negation operator so
/// it's exactly one space, closing the gap [`check`] flags (inserting a
/// space where there was none, and collapsing a longer run of spaces/tabs
/// down to one). A `!` on a line protected by a CreationKit fragment-code
/// wrapper (see [`fragment_code`]), or with nothing but a line ending/end of
/// file after it, is left exactly as-is — see [`check`].
pub fn repair(source: &str) -> String {
    let protected = fragment_code::protected_lines(source);
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Not || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let (start, end) = whitespace_run(bytes, offset);
        if starts_another_negation(bytes, start, end) {
            continue;
        }
        if !at_end_of_line(bytes, end) && !is_single_space(bytes, start, end) {
            ranges.push((start, end));
        }
    }

    if ranges.is_empty() {
        return source.to_string();
    }

    let mut repaired = String::with_capacity(source.len() + ranges.len());
    let mut previous = 0;
    for (start, end) in ranges {
        repaired.push_str(&source[previous..start]);
        repaired.push(' ');
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

/// The `[start, end)` byte range of the run of spaces/tabs immediately
/// following the `!` at `offset`.
fn whitespace_run(bytes: &[u8], offset: usize) -> (usize, usize) {
    let start = offset + 1;
    let mut end = start;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    (start, end)
}

fn is_single_space(bytes: &[u8], start: usize, end: usize) -> bool {
    end - start == 1 && bytes.get(start) == Some(&b' ')
}

/// Whether the `!` whose whitespace run is `[start, end)` is immediately
/// followed by another `!` with no space in between, i.e. `start == end`
/// (no whitespace at all was found) and the very next byte starts another
/// negation.
fn starts_another_negation(bytes: &[u8], start: usize, end: usize) -> bool {
    start == end && bytes.get(end) == Some(&b'!')
}

/// Whether `end` sits at the end of the line (a `\n`/`\r`) or end of file,
/// meaning the whitespace run examined by [`whitespace_run`] found nothing
/// but a line ending after it.
fn at_end_of_line(bytes: &[u8], end: usize) -> bool {
    !matches!(bytes.get(end), Some(byte) if *byte != b'\n' && *byte != b'\r')
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_a_single_space() {
        let source =
            "ScriptName Example\n\nFunction Test()\n    If ! bReady\n    EndIf\nEndFunction\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn flags_no_space() {
        let diagnostics = check("If !bReady\nEndIf\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert!(diagnostics[0].message.starts_with("[warning]"));
    }

    #[test]
    fn flags_multiple_spaces() {
        let diagnostics = check("If !   bReady\nEndIf\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_tab() {
        let diagnostics = check("If !\tbReady\nEndIf\n");
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn ignores_not_equal_operator() {
        assert!(check("If a != b\nEndIf\n").is_empty());
    }

    #[test]
    fn ignores_exclamation_marks_in_strings_and_comments() {
        let source = "\
String message = \"Look!NoSpace\"
; !comment
{/ !documentation /}
;/ !block
comment /;
";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn malformed_source_is_left_unchanged() {
        // Lints inspect incomplete scripts where possible, but a lexer error
        // must not lead to a partial or incorrectly positioned edit.
        let source = "If !bReady\n    String message = \"unterminated\n";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn ignores_negation_with_nothing_but_a_newline_after_it() {
        // Inserting a space here would just be trailing whitespace, which
        // the "Trailing whitespace" fix would strip right back off in the
        // combined `repair()` pipeline (it runs after this one), so this
        // lint has nothing useful to say about it.
        assert!(check("If !\nEndIf\n").is_empty());
    }

    #[test]
    fn ignores_negation_with_only_trailing_whitespace_after_it() {
        assert!(check("If !   \nEndIf\n").is_empty());
    }

    #[test]
    fn ignores_negation_with_only_trailing_whitespace_before_crlf() {
        let source = "If !\t  \r\nEndIf\r\n";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn ignores_negation_at_end_of_file_with_no_trailing_newline() {
        assert!(check("If !").is_empty());
    }

    #[test]
    fn flags_each_negation_independently() {
        let diagnostics = check("If !a || !  b\nEndIf\n");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn ignores_the_gap_between_chained_negation_operators() {
        // `!!bReady` is two `!` tokens back to back; only the last one in
        // the run needs its own trailing space, so exactly one diagnostic
        // is expected here, for the second `!`.
        let diagnostics = check("If !!bReady\nEndIf\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 5);
    }

    #[test]
    fn ignores_the_gap_in_a_longer_run_of_chained_negation_operators() {
        let diagnostics = check("If !!!bReady\nEndIf\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 6);
    }

    #[test]
    fn ignores_a_chained_negation_run_already_followed_by_a_single_space() {
        assert!(check("If !! bReady\nEndIf\n").is_empty());
    }

    #[test]
    fn fragment_code_wrapper_negations_are_left_alone() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If !bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        let diagnostics = check(source);
        assert!(diagnostics.iter().all(|d| d.line == 4));
        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn repair_leaves_a_single_space_untouched() {
        let source = "If ! bReady\nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_inserts_a_missing_space() {
        assert_eq!(repair("If !bReady\nEndIf\n"), "If ! bReady\nEndIf\n");
    }

    #[test]
    fn repair_collapses_multiple_spaces() {
        assert_eq!(repair("If !   bReady\nEndIf\n"), "If ! bReady\nEndIf\n");
    }

    #[test]
    fn repair_replaces_a_tab_with_a_space() {
        assert_eq!(repair("If !\tbReady\nEndIf\n"), "If ! bReady\nEndIf\n");
    }

    #[test]
    fn repair_leaves_not_equal_operator_alone() {
        let source = "If a != b\nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_negation_with_nothing_but_a_newline_after_it_alone() {
        let source = "If !\nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_negation_with_only_trailing_whitespace_after_it_alone() {
        let source = "If !   \nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_negation_at_end_of_file_with_no_trailing_newline_alone() {
        let source = "If !";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_fixes_each_negation_independently() {
        assert_eq!(repair("If !a || !  b\nEndIf\n"), "If ! a || ! b\nEndIf\n");
    }

    #[test]
    fn repair_leaves_the_gap_between_chained_negation_operators_alone() {
        assert_eq!(repair("If !!bReady\nEndIf\n"), "If !! bReady\nEndIf\n");
    }

    #[test]
    fn repair_leaves_a_longer_run_of_chained_negation_operators_alone() {
        assert_eq!(repair("If !!!bReady\nEndIf\n"), "If !!! bReady\nEndIf\n");
    }

    #[test]
    fn repair_leaves_an_already_correct_chained_negation_run_untouched() {
        let source = "If !! bReady\nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_fixes_the_code_body_but_leaves_the_wrapper_alone() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If !bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        assert_eq!(
            repair(source),
            "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If ! bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
        );
    }

    #[test]
    fn repair_result_has_no_remaining_diagnostics() {
        let source = "If !bReady || !  bOther\nEndIf\n";
        let repaired = repair(source);
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_is_idempotent() {
        let repaired = repair("If !   bReady\nEndIf\n");
        assert_eq!(repair(&repaired), repaired);
    }
}

//! Requires exactly one space on either side of the assignment (`=`, `+=`,
//! `-=`, `*=`, `/=`, `%=`) operators.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "assignment-operator-spacing";

/// The source text of a token this lint cares about, or `None` for every
/// other token kind.
fn operator_text(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::Assign => Some("="),
        TokenKind::PlusAssign => Some("+="),
        TokenKind::MinusAssign => Some("-="),
        TokenKind::StarAssign => Some("*="),
        TokenKind::SlashAssign => Some("/="),
        TokenKind::PercentAssign => Some("%="),
        _ => None,
    }
}

/// Checks for an assignment operator not surrounded by exactly one space on
/// a side that shares its physical line with the operator (a side whose
/// whitespace run reaches a newline — the operator opens or closes a
/// statement continued across lines — is never flagged on that side).
/// Operators on a line protected by a CreationKit fragment-code wrapper
/// (see [`fragment_code`]) are never flagged. `==`, `!=`, `>=`, and `<=`
/// are never matched here, since the lexer tokenizes those as their own,
/// separate token kinds (see [`crate::operator_spacing`] instead).
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
        let Some(text) = operator_text(&token.kind) else {
            continue;
        };
        if protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let end = offset + text.len();

        if let Some((start, len)) = leading_gap(bytes, offset) {
            if !(len == 1 && bytes[start] == b' ') {
                diagnostics.push(Diagnostic {
                    line: token.line,
                    column: token.col,
                    message: format!("[warning] '{text}' must be preceded by exactly one space"),
                    rule: RULE,
                });
            }
        }
        if let Some((start, gend)) = trailing_gap(bytes, end) {
            if !(gend - start == 1 && bytes[start] == b' ') {
                diagnostics.push(Diagnostic {
                    line: token.line,
                    column: token.col,
                    message: format!("[warning] '{text}' must be followed by exactly one space"),
                    rule: RULE,
                });
            }
        }
    }
    diagnostics
}

/// Normalizes the whitespace on either side of every assignment operator to
/// exactly one space, applying the same same-line rule (and fragment-code
/// exemption) as [`check`]. A gap that reaches a newline is left exactly
/// as-is, so a statement continued across physical lines keeps its own
/// line breaks.
pub fn repair(source: &str) -> String {
    let protected = fragment_code::protected_lines(source);
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut edits: Vec<(usize, usize)> = Vec::new();
    for token in tokens {
        let Some(text) = operator_text(&token.kind) else {
            continue;
        };
        if protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;
        let end = offset + text.len();

        if let Some((start, len)) = leading_gap(bytes, offset) {
            if !(len == 1 && bytes[start] == b' ') {
                edits.push((start, offset));
            }
        }
        if let Some((start, gend)) = trailing_gap(bytes, end) {
            if !(gend - start == 1 && bytes[start] == b' ') {
                edits.push((start, gend));
            }
        }
    }

    if edits.is_empty() {
        return source.to_string();
    }
    edits.sort_unstable();
    edits.dedup();

    let mut repaired = String::with_capacity(source.len());
    let mut previous = 0;
    for (start, end) in edits {
        let start = start.max(previous);
        if start > end {
            continue;
        }
        repaired.push_str(&source[previous..start]);
        repaired.push(' ');
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

/// True for a byte that ends a physical line (a newline, or nothing at
/// all, since `\r` in a `\r\n` ending always sits right before a `\n`).
fn is_line_boundary(byte: Option<u8>) -> bool {
    matches!(byte, None | Some(b'\n') | Some(b'\r'))
}

/// The contiguous run of spaces/tabs immediately before `offset`, as
/// `(start, length)`, unless that run reaches the start of the file or a
/// preceding newline — in which case `offset` opens a statement continued
/// from a previous physical line, which this lint leaves alone, and `None`
/// is returned.
fn leading_gap(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut start = offset;
    while start > 0 && (bytes[start - 1] == b' ' || bytes[start - 1] == b'\t') {
        start -= 1;
    }
    if start == 0 || bytes[start - 1] == b'\n' {
        return None;
    }
    Some((start, offset - start))
}

/// The contiguous run of spaces/tabs starting at `offset`, as
/// `(start, end)`, unless that run reaches the end of the file or a
/// following newline — in which case the statement continues onto the
/// next physical line, which this lint leaves alone, and `None` is
/// returned.
fn trailing_gap(bytes: &[u8], offset: usize) -> Option<(usize, usize)> {
    let mut end = offset;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if is_line_boundary(bytes.get(end).copied()) {
        return None;
    }
    Some((offset, end))
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
    fn ignores_correctly_spaced_operators() {
        let source = "\
ScriptName Example

Function Test(Int a, Int b)
    a = b
    a += b
    a -= b
    a *= b
    a /= b
    a %= b
EndFunction
";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn flags_missing_space_on_both_sides() {
        let diagnostics = check("a=b\n");
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("preceded")));
        assert!(diagnostics.iter().any(|d| d.message.contains("followed")));
    }

    #[test]
    fn flags_missing_space_before_only() {
        let diagnostics = check("a =b\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("followed"));
    }

    #[test]
    fn flags_missing_space_after_only() {
        let diagnostics = check("a= b\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("preceded"));
    }

    #[test]
    fn flags_extra_spaces_and_tabs() {
        let diagnostics = check("a  =\tb\n");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn checks_every_relevant_operator_kind() {
        for source in ["a+=b\n", "a-=b\n", "a*=b\n", "a/=b\n", "a%=b\n"] {
            let diagnostics = check(source);
            assert_eq!(diagnostics.len(), 2, "source: {source:?}");
        }
    }

    #[test]
    fn ignores_unrelated_operators() {
        assert!(
            check("If a==b\nEndIf\nIf a!=b\nEndIf\nIf a>=b\nEndIf\nIf a<=b\nEndIf\n").is_empty()
        );
    }

    #[test]
    fn does_not_flag_a_statement_continued_across_lines() {
        let source = "a \\\n    = b\n";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn fragment_code_wrapper_operators_are_left_alone() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(Int a,Int b)
;BEGIN CODE
a=b
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        let diagnostics = check(source);
        assert!(diagnostics.iter().all(|d| d.line == 4));
        assert_eq!(diagnostics.len(), 2);
        let repaired = repair(source);
        assert!(repaired.contains("a = b\n"));
        assert!(repaired.contains("Function Fragment_0(Int a,Int b)"));
    }

    #[test]
    fn repair_inserts_missing_spaces() {
        assert_eq!(repair("a=b\n"), "a = b\n");
    }

    #[test]
    fn repair_collapses_extra_spaces_and_tabs() {
        assert_eq!(repair("a  =\tb\n"), "a = b\n");
    }

    #[test]
    fn repair_normalizes_every_operator_in_a_longer_script() {
        assert_eq!(
            repair("a=b\na+=1\na-=1\na*=2\na/=2\na%=2\n"),
            "a = b\na += 1\na -= 1\na *= 2\na /= 2\na %= 2\n"
        );
    }

    #[test]
    fn repair_leaves_a_line_continuation_alone() {
        let source = "a \\\n    =b\n";
        assert_eq!(repair(source), "a \\\n    = b\n");
    }

    #[test]
    fn repair_does_not_touch_comparison_operators() {
        let source = "If a==b\nEndIf\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_is_idempotent() {
        let repaired = repair("a  =  b\na+=\t1\n");
        assert_eq!(repair(&repaired), repaired);
        assert!(check(&repaired).is_empty());
    }
}

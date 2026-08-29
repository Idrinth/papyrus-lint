//! Flags whitespace that interrupts a property or method access chain,
//! e.g. `SomeProperty . DoThing()` instead of `SomeProperty.DoThing()`.

use crate::{fragment_code, Diagnostic};
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::TokenKind;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "chain-whitespace";

const WHITESPACE: [u8; 2] = [b' ', b'\t'];

/// Checks for a `.` member/method access whose adjacent character, on
/// either side and on the same line, is a space or tab, since that
/// whitespace interrupts the chain for no benefit. Always reported as an
/// `[error]`. A dot inside a `Float` literal (e.g. `1.5`) is lexed as part
/// of the number itself and never reaches this check. Dots on a line
/// protected by a CreationKit fragment-code wrapper (see [`fragment_code`])
/// are never flagged.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let protected = fragment_code::protected_lines(source);
    let tokens = match Lexer::new(source).tokenize() {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut diagnostics = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Dot || protected[token.line] {
            continue;
        }

        let offset = line_starts[token.line - 1] + token.col - 1;

        let before = offset
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .copied();
        if before.is_some_and(|byte| WHITESPACE.contains(&byte)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[error] Whitespace before '.' interrupts property/method chaining"
                    .to_string(),
                rule: RULE,
            });
        }

        let after = bytes.get(offset + 1).copied();
        if after.is_some_and(|byte| WHITESPACE.contains(&byte)) {
            diagnostics.push(Diagnostic {
                line: token.line,
                column: token.col,
                message: "[error] Whitespace after '.' interrupts property/method chaining"
                    .to_string(),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// Removes whitespace immediately before/after a `.` member/method access,
/// closing the same gaps [`check`] flags (a dot inside a `Float` literal, or
/// on a line protected by a CreationKit fragment-code wrapper, is left
/// alone). Only the contiguous run of spaces/tabs touching the dot itself is
/// removed; anything past a newline (a chain continued onto another
/// physical line) is untouched, matching what [`check`] considers "the same
/// line".
pub fn repair(source: &str) -> String {
    let protected = fragment_code::protected_lines(source);
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let bytes = source.as_bytes();

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for token in tokens {
        if token.kind != TokenKind::Dot || protected[token.line] {
            continue;
        }
        let offset = line_starts[token.line - 1] + token.col - 1;

        let mut start = offset;
        while start > 0 && WHITESPACE.contains(&bytes[start - 1]) {
            start -= 1;
        }
        if start < offset {
            ranges.push((start, offset));
        }

        let mut end = offset + 1;
        while end < bytes.len() && WHITESPACE.contains(&bytes[end]) {
            end += 1;
        }
        if end > offset + 1 {
            ranges.push((offset + 1, end));
        }
    }

    if ranges.is_empty() {
        return source.to_string();
    }
    ranges.sort_unstable();

    let mut repaired = String::with_capacity(source.len());
    let mut previous = 0;
    for (start, end) in ranges {
        let start = start.max(previous);
        if start >= end {
            continue;
        }
        repaired.push_str(&source[previous..start]);
        previous = end;
    }
    repaired.push_str(&source[previous..]);
    repaired
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
    fn ignores_clean_chains() {
        let source = "ScriptName Example\n\nFunction Test()\n    SomeProperty.DoThing().Other()\nEndFunction\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn flags_whitespace_before_dot() {
        let diagnostics = check("SomeProperty .DoThing()\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert!(diagnostics[0].message.starts_with("[error]"));
        assert!(diagnostics[0].message.contains("before"));
    }

    #[test]
    fn flags_whitespace_after_dot() {
        let diagnostics = check("SomeProperty. DoThing()\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert!(diagnostics[0].message.contains("after"));
    }

    #[test]
    fn flags_whitespace_on_both_sides_as_two_diagnostics() {
        let diagnostics = check("SomeProperty . DoThing()\n");
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("before")));
        assert!(diagnostics.iter().any(|d| d.message.contains("after")));
    }

    #[test]
    fn flags_a_tab_the_same_as_a_space() {
        let diagnostics = check("SomeProperty\t.DoThing()\n");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("before"));
    }

    #[test]
    fn flags_each_interrupted_dot_in_a_longer_chain_independently() {
        let diagnostics = check("a. b.c .d\n");
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn ignores_float_literals() {
        assert!(
            check("ScriptName Example\n\nFunction Test()\n    Float x = 1.5\nEndFunction\n")
                .is_empty()
        );
    }

    #[test]
    fn fragment_code_wrapper_dots_are_left_alone() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef)
;BEGIN CODE
akSpeaker . RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        let diagnostics = check(source);
        assert!(diagnostics.iter().all(|d| d.line == 4));
        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn repair_leaves_clean_chains_untouched() {
        let source = "ScriptName Example\n\nFunction Test()\n    SomeProperty.DoThing().Other()\nEndFunction\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_closes_whitespace_before_a_dot() {
        assert_eq!(
            repair("SomeProperty .DoThing()\n"),
            "SomeProperty.DoThing()\n"
        );
    }

    #[test]
    fn repair_closes_whitespace_after_a_dot() {
        assert_eq!(
            repair("SomeProperty. DoThing()\n"),
            "SomeProperty.DoThing()\n"
        );
    }

    #[test]
    fn repair_closes_whitespace_on_both_sides() {
        assert_eq!(
            repair("SomeProperty . DoThing()\n"),
            "SomeProperty.DoThing()\n"
        );
    }

    #[test]
    fn repair_closes_runs_of_multiple_spaces_and_tabs() {
        assert_eq!(
            repair("SomeProperty  \t . \t  DoThing()\n"),
            "SomeProperty.DoThing()\n"
        );
    }

    #[test]
    fn repair_closes_every_interrupted_dot_in_a_longer_chain() {
        assert_eq!(repair("a. b.c .d\n"), "a.b.c.d\n");
    }

    #[test]
    fn repair_result_has_no_remaining_diagnostics() {
        let source = "a . b . c\n";
        let repaired = repair(source);
        assert_eq!(repaired, "a.b.c\n");
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_leaves_float_literals_alone() {
        let source = "ScriptName Example\n\nFunction Test()\n    Float x = 1.5\nEndFunction\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_fixes_the_code_body_but_leaves_the_wrapper_alone() {
        // Mirrors `fragment_code_wrapper_dots_are_left_alone` above: the
        // wrapper boilerplate (including the generated function signature,
        // deliberately given a space-interrupted default-value chain here to
        // prove it's left alone) must come out byte-for-byte identical, while
        // the actual code between `;BEGIN CODE`/`;END CODE` gets fixed.
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef = akRoot . GetRef())
;BEGIN CODE
akSpeaker . RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        assert_eq!(
            repair(source),
            "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef = akRoot . GetRef())
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
        );
    }

    #[test]
    fn repair_does_not_reach_across_a_newline() {
        // A chain continued onto another physical line (no trailing "\" line
        // continuation) is a different statement as far as the lexer is
        // concerned, not whitespace interrupting a single dot access.
        let source = "a\n.b\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_is_idempotent() {
        let repaired = repair("SomeProperty  .  DoThing() .Other()\n");
        assert_eq!(repair(&repaired), repaired);
    }
}

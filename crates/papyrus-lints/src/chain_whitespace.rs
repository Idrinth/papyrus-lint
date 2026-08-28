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
}

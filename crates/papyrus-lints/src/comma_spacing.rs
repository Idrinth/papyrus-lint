//! Requires whitespace after commas in parenthesized argument lists.

use crate::Diagnostic;
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::TokenKind;

/// Checks for argument-list commas that are immediately followed by another
/// non-whitespace character.
pub fn check(source: &str) -> Vec<Diagnostic> {
    comma_offsets(source)
        .into_iter()
        .map(|(_, line, column)| Diagnostic {
            line,
            column,
            message: "Comma in argument list must be followed by whitespace".to_string(),
        })
        .collect()
}

/// Inserts one space after every unspaced comma in an argument list.
pub fn repair(source: &str) -> String {
    let offsets: Vec<_> = comma_offsets(source)
        .into_iter()
        .map(|(offset, _, _)| offset)
        .collect();
    if offsets.is_empty() {
        return source.to_string();
    }

    let mut repaired = String::with_capacity(source.len() + offsets.len());
    let mut previous = 0;
    for offset in offsets {
        let after_comma = offset + 1;
        repaired.push_str(&source[previous..after_comma]);
        repaired.push(' ');
        previous = after_comma;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

fn comma_offsets(source: &str) -> Vec<(usize, usize, usize)> {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };
    let line_starts = line_starts(source);
    let mut paren_depth = 0usize;
    let mut commas = Vec::new();

    for token in tokens {
        match token.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::Comma if paren_depth > 0 => {
                let offset = line_starts[token.line - 1] + token.col - 1;
                let next = source.as_bytes().get(offset + 1).copied();
                if next.is_some_and(|byte| !byte.is_ascii_whitespace() && byte != b')') {
                    commas.push((offset, token.line, token.col));
                }
            }
            _ => {}
        }
    }
    commas
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
    fn flags_and_repairs_calls_and_declarations() {
        let source = "Function Add(Int left,Int right)\n  Use(Add(1,2),3)\nEndFunction\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 3);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 22));
        assert_eq!(
            repair(source),
            "Function Add(Int left, Int right)\n  Use(Add(1, 2), 3)\nEndFunction\n"
        );
    }

    #[test]
    fn accepts_existing_whitespace_and_multiline_arguments() {
        let source = "Use(1, 2,\t3,\n  4,\r\n  5)\n";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn ignores_commas_outside_argument_lists_and_inside_strings_or_comments() {
        let source = "String value = \"one,two\" ; comment,here\n; / block,comment /;\nInt[] values = [1,2]\n";
        assert!(check(source).is_empty());
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_is_idempotent_and_preserves_unicode() {
        let source = "Show(\"é\",value)\n";
        let repaired = repair(source);
        assert_eq!(repaired, "Show(\"é\", value)\n");
        assert_eq!(repair(&repaired), repaired);
    }
}

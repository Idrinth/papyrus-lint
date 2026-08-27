//! Flags getter calls used as standalone statements.

use crate::Diagnostic;
use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{Token, TokenKind};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-getter";

/// Checks for calls whose function name begins with `Get` and whose result is
/// discarded rather than assigned, returned, or used by another expression.
/// Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match Lexer::new(source).tokenize() {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    tokens
        .split(|token| matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .filter_map(check_statement)
        .collect()
}

/// Flags `statement` (the tokens between two newlines) if it is nothing but
/// a bare call to a `Get`-prefixed function, with no other operator or
/// keyword that would consume its return value.
fn check_statement(statement: &[Token]) -> Option<Diagnostic> {
    // A discarded expression cannot contain a statement keyword or an
    // assignment. This also excludes declarations, returns, and conditions.
    if statement.iter().any(|token| {
        matches!(
            token.kind,
            TokenKind::Keyword(_)
                | TokenKind::Assign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::StarAssign
                | TokenKind::SlashAssign
                | TokenKind::PercentAssign
        )
    }) {
        return None;
    }

    if has_top_level_operator(statement) {
        return None;
    }

    let last = statement.last()?;
    if !matches!(last.kind, TokenKind::RParen) {
        return None;
    }

    let open_index = matching_open_paren(statement, statement.len() - 1)?;
    let function = open_index.checked_sub(1).and_then(|index| {
        let token = &statement[index];
        match &token.kind {
            TokenKind::Identifier(name) => Some((token, name)),
            _ => None,
        }
    })?;

    if !function
        .1
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("get"))
    {
        return None;
    }

    Some(Diagnostic {
        line: function.0.line,
        column: function.0.col,
        message: format!(
            "[warning] Getter '{}' is called without using its return value",
            function.1
        ),
        rule: RULE,
    })
}

fn has_top_level_operator(tokens: &[Token]) -> bool {
    let mut paren_depth: usize = 0;
    let mut bracket_depth: usize = 0;

    for token in tokens {
        match token.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Gt
            | TokenKind::Lt
            | TokenKind::GtEq
            | TokenKind::LtEq
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Not
                if paren_depth == 0 && bracket_depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }

    false
}

fn matching_open_paren(tokens: &[Token], close_index: usize) -> Option<usize> {
    let mut depth = 0;
    for index in (0..=close_index).rev() {
        match tokens[index].kind {
            TokenKind::RParen => depth += 1,
            TokenKind::LParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_discarded_qualified_and_unqualified_getters_case_insensitively() {
        let diagnostics = check(
            "Function Test()\n  GetValue()\n  object.gEtOtherValue(1, NestedCall())\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
        assert_eq!((diagnostics[1].line, diagnostics[1].column), (3, 10));
    }

    #[test]
    fn ignores_getter_results_that_are_used() {
        let diagnostics = check(
            "Function Test()\n  Int value = GetValue()\n  value = GetValue()\n  Return GetValue()\n  UseValue(GetValue())\n  GetValue().UseValue()\n  Bool equal = other == GetValue()\n  other == GetValue()\n  !GetValue()\n  If GetValue()\n  EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_discarded_non_getter_calls_and_getter_declarations() {
        let diagnostics = check(
            "Int Function GetValue()\n  DoSomething()\n  ForgetSomething()\n  Return 1\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn supports_multiline_calls() {
        let diagnostics = check("Function Test()\n  GetValue(\\\n    1)\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn flags_the_final_getter_in_a_getter_chain() {
        let diagnostics = check(
            "Function Test()\n  Game.GetPlayer().GetActorBase().GetAV(\"health\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert!(diagnostics[0].message.contains("GetAV"));
    }
}

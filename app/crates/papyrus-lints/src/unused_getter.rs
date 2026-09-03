//! Flags getter calls used as standalone statements.

use crate::Diagnostic;
use papyrus_parser::token::{Token, TokenKind};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-getter";

/// Checks for calls whose function name begins with `Get` and whose result is
/// discarded rather than assigned, returned, or used by another expression.
/// Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    tokens
        .split(|token| matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .filter_map(check_statement)
        .collect()
}

/// Flags `statement` (the tokens between two newlines) if any top-level
/// operand of its expression is nothing but a bare call to a
/// `Get`-prefixed function, with no keyword or assignment that would
/// consume the statement's overall result. A top-level operand whose value
/// only feeds a comparison, arithmetic, or logical operator is still
/// flagged, since the operator's own result is then itself discarded (e.g.
/// `GetDistance(target) > 0` on its own line).
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

    top_level_operands(statement)
        .into_iter()
        .find_map(check_operand)
}

/// Checks whether `operand` (one top-level operand of a discarded
/// expression statement, as split out by [`top_level_operands`]) is itself
/// nothing but a call to a `Get`-prefixed function.
fn check_operand(operand: &[Token]) -> Option<Diagnostic> {
    let last = operand.last()?;
    if !matches!(last.kind, TokenKind::RParen) {
        return None;
    }

    let open_index = matching_open_paren(operand, operand.len() - 1)?;
    let function = open_index.checked_sub(1).and_then(|index| {
        let token = &operand[index];
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

/// Splits `tokens` into the operands of any top-level comparison,
/// arithmetic, or logical operator (i.e. one at parenthesis/bracket depth
/// zero), dropping the operators themselves. A statement with no such
/// operator yields a single operand equal to the whole statement,
/// preserving the previous (pre-operator-aware) behavior for a bare call.
fn top_level_operands(tokens: &[Token]) -> Vec<&[Token]> {
    let mut operands = Vec::new();
    let mut start = 0;
    let mut paren_depth: usize = 0;
    let mut bracket_depth: usize = 0;

    for (index, token) in tokens.iter().enumerate() {
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
                if index > start {
                    operands.push(&tokens[start..index]);
                }
                start = index + 1;
            }
            _ => {}
        }
    }

    if start < tokens.len() {
        operands.push(&tokens[start..]);
    }

    operands
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
            "Function Test()\n  Int value = GetValue()\n  value = GetValue()\n  Return GetValue()\n  UseValue(GetValue())\n  GetValue().UseValue()\n  Bool equal = other == GetValue()\n  If GetValue()\n  EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_getter_whose_result_only_feeds_a_discarded_comparison() {
        // The comparison consumes GetDistance's return value, but the
        // comparison's own result is then discarded too, since the
        // statement is neither an assignment, a return, nor a condition.
        let diagnostics = check(
            "ScriptName ABC extends Actor\n\nActor Property B Auto\n\nFunction A()\n   GetDistance(B) > 0\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert!(diagnostics[0].message.contains("GetDistance"));
    }

    #[test]
    fn flags_getters_discarded_through_a_negation_or_equality_check() {
        let diagnostics =
            check("Function Test()\n  !GetValue()\n  other == GetValue()\nEndFunction\n");

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|d| d.message.contains("GetValue")));
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

    #[test]
    fn flags_only_the_first_getter_in_one_discarded_compound_expression() {
        let diagnostics =
            check("Function Test()\n  GetFirst() + GetSecond() * GetThird()\nEndFunction\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
        assert!(diagnostics[0].message.contains("GetFirst"));
    }

    #[test]
    fn operators_inside_call_arguments_do_not_hide_a_discarded_getter() {
        let diagnostics = check(
            "Function Test(Int Left, Int Right)\n  GetValue((Left + Right) * 2)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
        assert!(diagnostics[0].message.contains("GetValue"));
    }

    #[test]
    fn ignores_getters_consumed_by_compound_assignments() {
        let diagnostics = check(
            "Function Test()\n  Value += GetValue()\n  Value -= GetValue()\n  Value *= GetValue()\n  Value /= GetValue()\n  Value %= GetValue()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_get_prefixed_identifiers_that_are_not_calls() {
        let diagnostics = check(
            "Function Test()\n  GetValue\n  object.GetValue\n  values[GetIndex()]\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }
}

//! Flags getter calls used as standalone statements.

use papyrus_parser::token::{Token, TokenKind};

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use crate::token_walk::{matching_open_paren, top_level_operands};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-getter";

#[derive(Default)]
struct Collect {
    store: Store,
    statement: Vec<Token>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_token(
        &mut self,
        token: &Token,
        _index: usize,
        _tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        if matches!(token.kind, TokenKind::Newline | TokenKind::Eof) {
            self.flush_statement();
            return;
        }
        self.statement.push(token.clone());
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        self.flush_statement();
    }
}

impl Collect {
    fn flush_statement(&mut self) {
        if let Some(diagnostic) = check_statement(&self.statement) {
            self.store.push(diagnostic);
        }
        self.statement.clear();
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks for calls whose function name begins with `Get` and whose result is
/// discarded rather than assigned, returned, or used by another expression.
/// Flagged as a `[warning]`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
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

#[cfg(test)]
#[path = "unused_getter_tests.rs"]
mod tests;

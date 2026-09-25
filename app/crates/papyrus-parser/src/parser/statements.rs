use super::{PResult, Parser};
use crate::ast::*;
use crate::token::{Keyword, TokenKind};

impl Parser {
    // ---- statements ------------------------------------------------------

    pub(super) fn parse_block(&mut self, end_keywords: &[Keyword]) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            if end_keywords.iter().any(|kw| self.at_keyword(*kw)) {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        let line = self.current().line;

        if self.at_keyword(Keyword::If) {
            return self.parse_if();
        }
        if self.at_keyword(Keyword::While) {
            return self.parse_while();
        }
        if self.at_keyword(Keyword::LockGuard) || self.at_keyword(Keyword::TryLockGuard) {
            if !self.mode.has_starfield_dialect() {
                return Err(self.error("LockGuard and TryLockGuard are Starfield statements"));
            }
            let is_try = self.at_keyword(Keyword::TryLockGuard);
            return self.parse_lock_guard(is_try);
        }
        if self.at_keyword(Keyword::Return) {
            self.advance();
            let value = if matches!(
                self.kind(),
                TokenKind::Newline | TokenKind::CommentAnnotation(_)
            ) || self.is_eof()
            {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_terminator()?;
            return Ok(Stmt::Return { value, line });
        }

        if self.looks_like_var_decl() {
            let type_name = self.parse_type_name()?;
            let name = self.expect_identifier()?;
            return Ok(Stmt::VarDecl(
                self.parse_variable_tail(type_name, name, line)?,
            ));
        }

        let target = self.parse_expr()?;
        let op = match self.kind() {
            TokenKind::Assign => Some(AssignOp::Assign),
            TokenKind::PlusAssign => Some(AssignOp::AddAssign),
            TokenKind::MinusAssign => Some(AssignOp::SubAssign),
            TokenKind::StarAssign => Some(AssignOp::MulAssign),
            TokenKind::SlashAssign => Some(AssignOp::DivAssign),
            TokenKind::PercentAssign => Some(AssignOp::ModAssign),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let value = self.parse_expr()?;
            self.expect_terminator()?;
            return Ok(Stmt::Assign {
                target,
                op,
                value,
                line,
            });
        }

        self.expect_terminator()?;
        Ok(Stmt::Expr {
            value: target,
            line,
        })
    }

    /// Disambiguates a local variable declaration (`Type name = ...`) from an
    /// expression statement / assignment by looking ahead for the
    /// `Identifier [ ':' Identifier ]* [ '[' ']' ] Identifier` pattern,
    /// without consuming tokens. Colon segments are part of a type name
    /// only in Fallout 4 / Starfield mode.
    fn looks_like_var_decl(&self) -> bool {
        let mut i = self.pos;
        if !matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::Identifier(_))
        ) {
            return false;
        }
        i += 1;
        if self.mode.has_fallout4_dialect() {
            while matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Colon))
                && matches!(
                    self.tokens.get(i + 1).map(|t| &t.kind),
                    Some(TokenKind::Identifier(_))
                )
            {
                i += 2;
            }
        }
        if matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::LBracket)
        ) && matches!(
            self.tokens.get(i + 1).map(|t| &t.kind),
            Some(TokenKind::RBracket)
        ) {
            i += 2;
        }
        matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::Identifier(_))
        )
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let line = self.current().line;
        let col = self.current().col;
        self.expect_keyword(Keyword::If)?;
        let mut branches = Vec::new();
        let condition = self.parse_expr()?;
        self.expect_terminator()?;
        let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
        branches.push(IfBranch {
            condition,
            body,
            line,
            col,
        });

        while self.at_keyword(Keyword::ElseIf) {
            let line = self.current().line;
            let col = self.current().col;
            self.advance();
            let condition = self.parse_expr()?;
            self.expect_terminator()?;
            let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
            branches.push(IfBranch {
                condition,
                body,
                line,
                col,
            });
        }

        let (else_body, else_line, else_col) = if self.at_keyword(Keyword::Else) {
            let else_line = self.current().line;
            let else_col = self.current().col;
            self.advance();
            self.expect_terminator()?;
            let body = self.parse_block(&[Keyword::EndIf])?;
            (body, Some(else_line), Some(else_col))
        } else {
            (Vec::new(), None, None)
        };

        self.expect_keyword(Keyword::EndIf)?;
        self.expect_terminator()?;

        Ok(Stmt::If {
            branches,
            else_body,
            else_line,
            else_col,
            line,
        })
    }

    fn parse_while(&mut self) -> PResult<Stmt> {
        let line = self.current().line;
        let col = self.current().col;
        self.expect_keyword(Keyword::While)?;
        let condition = self.parse_expr()?;
        self.expect_terminator()?;
        let body = self.parse_block(&[Keyword::EndWhile])?;
        self.expect_keyword(Keyword::EndWhile)?;
        self.expect_terminator()?;
        Ok(Stmt::While {
            condition,
            body,
            line,
            col,
        })
    }

    /// Starfield only. `LockGuard <Name>[, <Name>...]` /
    /// `LockGuard(<Name>[, <Name>...])` .. `EndLockGuard`, or
    /// `TryLockGuard <Name>[, <Name>...]` /
    /// `ElseTryLockGuard` / `Else` .. `EndTryLockGuard`.
    /// Only called when [`GameEdition::has_starfield_dialect`] is set.
    fn parse_lock_guard(&mut self, is_try: bool) -> PResult<Stmt> {
        let line = self.current().line;
        let col = self.current().col;
        if is_try {
            self.expect_keyword(Keyword::TryLockGuard)?;
        } else {
            self.expect_keyword(Keyword::LockGuard)?;
        }
        let parenthesized = matches!(self.kind(), TokenKind::LParen);
        if parenthesized {
            self.advance();
        }
        let mut names = vec![self.expect_identifier()?];
        while matches!(self.kind(), TokenKind::Comma) {
            self.advance();
            names.push(self.expect_identifier()?);
        }
        if parenthesized {
            self.expect(TokenKind::RParen)?;
        }
        self.expect_terminator()?;

        let (body, else_body, else_line, else_col) = if is_try {
            let body = self.parse_block(&[
                Keyword::ElseTryLockGuard,
                Keyword::Else,
                Keyword::EndTryLockGuard,
            ])?;
            let (else_body, else_line, else_col) =
                if self.at_keyword(Keyword::ElseTryLockGuard) || self.at_keyword(Keyword::Else) {
                    let else_line = self.current().line;
                    let else_col = self.current().col;
                    self.advance();
                    self.expect_terminator()?;
                    let else_body = self.parse_block(&[Keyword::EndTryLockGuard])?;
                    (else_body, Some(else_line), Some(else_col))
                } else {
                    (Vec::new(), None, None)
                };
            self.expect_keyword(Keyword::EndTryLockGuard)?;
            (body, else_body, else_line, else_col)
        } else {
            let body = self.parse_block(&[Keyword::EndLockGuard])?;
            self.expect_keyword(Keyword::EndLockGuard)?;
            (body, Vec::new(), None, None)
        };
        self.expect_terminator()?;

        Ok(Stmt::LockGuard {
            kind: if is_try {
                LockKind::Try
            } else {
                LockKind::Lock
            },
            names,
            body,
            else_body,
            else_line,
            else_col,
            line,
            col,
        })
    }
}

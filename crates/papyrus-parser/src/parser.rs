//! Recursive-descent parser producing an AST from a token stream.

use super::ast::*;
use super::token::{Keyword, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

type PResult<T> = Result<T, ParseError>;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn is_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let tok = self.current();
        ParseError {
            message: message.into(),
            line: tok.line,
            col: tok.col,
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    /// Consumes a single statement terminator (newline or end of file).
    fn expect_terminator(&mut self) -> PResult<()> {
        if self.is_eof() {
            return Ok(());
        }
        if matches!(self.kind(), TokenKind::Newline) {
            self.advance();
            return Ok(());
        }
        Err(self.error(format!("expected end of line, found {:?}", self.kind())))
    }

    fn expect_keyword(&mut self, kw: Keyword) -> PResult<Token> {
        if matches!(self.kind(), TokenKind::Keyword(k) if *k == kw) {
            Ok(self.advance())
        } else {
            Err(self.error(format!(
                "expected keyword {:?}, found {:?}",
                kw,
                self.kind()
            )))
        }
    }

    fn at_keyword(&self, kw: Keyword) -> bool {
        matches!(self.kind(), TokenKind::Keyword(k) if *k == kw)
    }

    fn expect_identifier(&mut self) -> PResult<String> {
        match self.kind().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(self.error(format!("expected identifier, found {:?}", other))),
        }
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
        if *self.kind() == kind {
            Ok(self.advance())
        } else {
            Err(self.error(format!("expected {:?}, found {:?}", kind, self.kind())))
        }
    }

    // ---- top level -----------------------------------------------------

    pub fn parse_script(&mut self) -> PResult<Script> {
        self.skip_newlines();
        self.expect_keyword(Keyword::ScriptName)?;
        let name = self.expect_identifier()?;

        let mut extends = None;
        if self.at_keyword(Keyword::Extends) {
            self.advance();
            extends = Some(self.expect_identifier()?);
        }

        let mut is_hidden = false;
        let mut is_conditional = false;
        loop {
            if self.at_keyword(Keyword::Hidden) {
                self.advance();
                is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                is_conditional = true;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        let mut script = Script {
            name,
            extends,
            is_hidden,
            is_conditional,
            imports: Vec::new(),
            properties: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            states: Vec::new(),
        };

        loop {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            self.parse_member(&mut script)?;
        }

        Ok(script)
    }

    fn parse_member(&mut self, script: &mut Script) -> PResult<()> {
        if self.at_keyword(Keyword::Import) {
            self.advance();
            let name = self.expect_identifier()?;
            self.expect_terminator()?;
            script.imports.push(name);
            return Ok(());
        }

        if self.at_keyword(Keyword::State) {
            script.states.push(self.parse_state(false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Auto) {
            self.advance();
            script.states.push(self.parse_state(true)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Function) {
            script.functions.push(self.parse_function(None, false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Event) {
            script.functions.push(self.parse_function(None, true)?);
            return Ok(());
        }

        let line = self.current().line;
        let type_name = self.parse_type_name()?;

        if self.at_keyword(Keyword::Function) {
            script
                .functions
                .push(self.parse_function(Some(type_name), false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Property) {
            script
                .properties
                .push(self.parse_property(type_name, line)?);
            return Ok(());
        }

        let name = self.expect_identifier()?;
        script
            .variables
            .push(self.parse_variable_tail(type_name, name, line)?);
        Ok(())
    }

    fn parse_type_name(&mut self) -> PResult<TypeName> {
        let name = self.expect_identifier()?;
        let mut is_array = false;
        if matches!(self.kind(), TokenKind::LBracket) {
            self.advance();
            self.expect(TokenKind::RBracket)?;
            is_array = true;
        }
        Ok(TypeName { name, is_array })
    }

    fn parse_property(&mut self, type_name: TypeName, line: usize) -> PResult<PropertyDecl> {
        self.expect_keyword(Keyword::Property)?;
        let name = self.expect_identifier()?;

        let mut value = None;
        if matches!(self.kind(), TokenKind::Assign) {
            self.advance();
            value = Some(self.parse_expr()?);
        }

        let mut is_auto = false;
        let mut is_auto_read_only = false;
        let mut is_hidden = false;
        let mut is_conditional = false;
        loop {
            if self.at_keyword(Keyword::Auto) {
                self.advance();
                is_auto = true;
            } else if self.at_keyword(Keyword::AutoReadOnly) {
                self.advance();
                is_auto_read_only = true;
            } else if self.at_keyword(Keyword::Hidden) {
                self.advance();
                is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                is_conditional = true;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        if !is_auto && !is_auto_read_only {
            // Full property: skip the Function/EndFunction get/set block(s);
            // parsing their bodies is out of scope for the basic AST.
            while !self.at_keyword(Keyword::EndProperty) && !self.is_eof() {
                self.advance();
            }
            self.expect_keyword(Keyword::EndProperty)?;
            self.expect_terminator()?;
        }

        Ok(PropertyDecl {
            type_name,
            name,
            value,
            is_auto,
            is_auto_read_only,
            is_hidden,
            is_conditional,
            line,
        })
    }

    fn parse_variable_tail(
        &mut self,
        type_name: TypeName,
        name: String,
        line: usize,
    ) -> PResult<VariableDecl> {
        let mut value = None;
        if matches!(self.kind(), TokenKind::Assign) {
            self.advance();
            value = Some(self.parse_expr()?);
        }
        let mut is_conditional = false;
        if self.at_keyword(Keyword::Conditional) {
            self.advance();
            is_conditional = true;
        }
        self.expect_terminator()?;
        Ok(VariableDecl {
            type_name,
            name,
            value,
            is_conditional,
            line,
        })
    }

    fn parse_state(&mut self, is_auto: bool) -> PResult<StateDecl> {
        let line = self.current().line;
        self.expect_keyword(Keyword::State)?;
        let name = self.expect_identifier()?;
        self.expect_terminator()?;

        let mut functions = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_keyword(Keyword::EndState) {
                break;
            }
            if self.is_eof() {
                return Err(self.error("expected EndState, found end of file"));
            }
            if self.at_keyword(Keyword::Function) {
                functions.push(self.parse_function(None, false)?);
                continue;
            }
            if self.at_keyword(Keyword::Event) {
                functions.push(self.parse_function(None, true)?);
                continue;
            }
            let return_type = self.parse_type_name()?;
            functions.push(self.parse_function(Some(return_type), false)?);
        }
        self.expect_keyword(Keyword::EndState)?;
        self.expect_terminator()?;

        Ok(StateDecl {
            name,
            is_auto,
            functions,
            line,
        })
    }

    fn parse_function(
        &mut self,
        return_type: Option<TypeName>,
        is_event: bool,
    ) -> PResult<FunctionDecl> {
        let line = self.current().line;
        if is_event {
            self.expect_keyword(Keyword::Event)?;
        } else {
            self.expect_keyword(Keyword::Function)?;
        }
        let name = self.expect_identifier()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        let mut is_global = false;
        let mut is_native = false;
        loop {
            if self.at_keyword(Keyword::Global) {
                self.advance();
                is_global = true;
            } else if self.at_keyword(Keyword::Native) {
                self.advance();
                is_native = true;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        let mut body = Vec::new();
        if !is_native {
            let end_kw = if is_event {
                Keyword::EndEvent
            } else {
                Keyword::EndFunction
            };
            body = self.parse_block(&[end_kw])?;
            self.expect_keyword(end_kw)?;
            self.expect_terminator()?;
        }

        Ok(FunctionDecl {
            name,
            return_type,
            params,
            is_global,
            is_native,
            is_event,
            body,
            line,
        })
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let type_name = self.parse_type_name()?;
            let name = self.expect_identifier()?;
            let mut default = None;
            if matches!(self.kind(), TokenKind::Assign) {
                self.advance();
                default = Some(self.parse_expr()?);
            }
            params.push(Param {
                type_name,
                name,
                default,
            });
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }
        Ok(params)
    }

    // ---- statements ------------------------------------------------------

    fn parse_block(&mut self, end_keywords: &[Keyword]) -> PResult<Vec<Stmt>> {
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
        if self.at_keyword(Keyword::Return) {
            self.advance();
            let value = if matches!(self.kind(), TokenKind::Newline) || self.is_eof() {
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
        Ok(Stmt::Expr(target))
    }

    /// Disambiguates a local variable declaration (`Type name = ...`) from an
    /// expression statement / assignment by looking ahead for the
    /// `Identifier [ '[' ']' ] Identifier` pattern, without consuming tokens.
    fn looks_like_var_decl(&self) -> bool {
        let mut i = self.pos;
        if !matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::Identifier(_))
        ) {
            return false;
        }
        i += 1;
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
        self.expect_keyword(Keyword::If)?;
        let mut branches = Vec::new();
        let condition = self.parse_expr()?;
        self.expect_terminator()?;
        let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
        branches.push(IfBranch { condition, body });

        while self.at_keyword(Keyword::ElseIf) {
            self.advance();
            let condition = self.parse_expr()?;
            self.expect_terminator()?;
            let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
            branches.push(IfBranch { condition, body });
        }

        let else_body = if self.at_keyword(Keyword::Else) {
            self.advance();
            self.expect_terminator()?;
            self.parse_block(&[Keyword::EndIf])?
        } else {
            Vec::new()
        };

        self.expect_keyword(Keyword::EndIf)?;
        self.expect_terminator()?;

        Ok(Stmt::If {
            branches,
            else_body,
            line,
        })
    }

    fn parse_while(&mut self) -> PResult<Stmt> {
        let line = self.current().line;
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
        })
    }

    // ---- expressions -------------------------------------------------

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut left = self.parse_and()?;
        while matches!(self.kind(), TokenKind::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut left = self.parse_equality()?;
        while matches!(self.kind(), TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> PResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.kind() {
                TokenKind::Eq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> PResult<Expr> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.kind() {
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::GtEq => BinaryOp::GtEq,
                TokenKind::LtEq => BinaryOp::LtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> PResult<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.kind() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> PResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.kind() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let op = match self.kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Not => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
            });
        }
        self.parse_cast()
    }

    fn parse_cast(&mut self) -> PResult<Expr> {
        let mut left = self.parse_postfix()?;
        while self.at_keyword(Keyword::As) {
            self.advance();
            let type_name = self.expect_identifier()?;
            left = Expr::Cast {
                value: Box::new(left),
                type_name,
            };
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.kind() {
                TokenKind::Dot => {
                    self.advance();
                    let property = self.expect_identifier()?;
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                TokenKind::LParen => {
                    let line = self.current().line;
                    let col = self.current().col;
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(TokenKind::RParen)?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        line,
                        col,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> PResult<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.current().clone();
        match tok.kind {
            TokenKind::IntLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(v)))
            }
            TokenKind::FloatLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(v)))
            }
            TokenKind::StringLiteral(ref v) => {
                let v = v.clone();
                self.advance();
                Ok(Expr::Literal(Literal::String(v)))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            TokenKind::Keyword(Keyword::None) => {
                self.advance();
                Ok(Expr::Literal(Literal::None))
            }
            TokenKind::Keyword(Keyword::Self_) => {
                self.advance();
                Ok(Expr::Self_)
            }
            TokenKind::Keyword(Keyword::Parent) => {
                self.advance();
                Ok(Expr::Parent)
            }
            TokenKind::Keyword(Keyword::New) => {
                self.advance();
                let name = self.expect_identifier()?;
                self.expect(TokenKind::LBracket)?;
                let size = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::NewArray {
                    type_name: TypeName {
                        name,
                        is_array: false,
                    },
                    size: Box::new(size),
                })
            }
            TokenKind::Identifier(ref name) => {
                let name = name.clone();
                self.advance();
                Ok(Expr::Identifier(name))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            other => Err(self.error(format!("unexpected token {:?}", other))),
        }
    }
}

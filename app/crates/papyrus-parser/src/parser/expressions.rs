use super::{PResult, Parser};
use crate::ast::*;
use crate::token::{Keyword, TokenKind};

impl Parser {
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
        loop {
            if self.at_keyword(Keyword::As) {
                self.advance();
                let type_name = self.parse_type_name()?;
                let type_name = if type_name.is_array {
                    format!("{}[]", type_name.name)
                } else {
                    type_name.name
                };
                left = Expr::Cast {
                    value: Box::new(left),
                    type_name,
                };
                continue;
            }
            if self.mode.has_fallout4_dialect() && self.at_keyword(Keyword::Is) {
                self.advance();
                let type_name = self.expect_qualified_name()?;
                left = Expr::Is {
                    value: Box::new(left),
                    type_name,
                };
                continue;
            }
            break;
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.kind() {
                TokenKind::Dot => {
                    self.advance();
                    let property = self.expect_property_name()?;
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
            args.push(self.parse_arg()?);
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }
        Ok(args)
    }

    /// Parses one call argument: either a plain expression, or a named
    /// argument (`name = value`), Papyrus's syntax for passing an argument
    /// by parameter name instead of by position.
    fn parse_arg(&mut self) -> PResult<Expr> {
        let name = match self.kind().clone() {
            TokenKind::Identifier(name) => Some(name),
            TokenKind::Keyword(Keyword::Hidden) => Some("hidden".to_string()),
            TokenKind::Keyword(Keyword::Conditional) => Some("conditional".to_string()),
            _ => None,
        };
        if let Some(name) = name {
            if matches!(self.peek_kind(1), TokenKind::Assign) {
                self.advance();
                self.advance();
                let value = self.parse_expr()?;
                return Ok(Expr::NamedArg {
                    name,
                    value: Box::new(value),
                });
            }
        }
        self.parse_expr()
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.current().clone();
        match tok.kind {
            TokenKind::IntLiteral(v, format) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int { value: v, format }))
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
            TokenKind::Keyword(Keyword::Hidden) => {
                self.advance();
                Ok(Expr::Identifier("hidden".to_string()))
            }
            TokenKind::Keyword(Keyword::Conditional) => {
                self.advance();
                Ok(Expr::Identifier("conditional".to_string()))
            }
            TokenKind::Keyword(Keyword::New) => {
                self.advance();
                let name = self.expect_qualified_name()?;
                if self.mode.has_fallout4_dialect() && !matches!(self.kind(), TokenKind::LBracket) {
                    // Fallout 4 only: `New <StructName>`, creating a struct
                    // instance rather than an array.
                    return Ok(Expr::NewStruct { type_name: name });
                }
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
                let name = if self.mode.has_fallout4_dialect() {
                    self.append_colon_segments(name)?
                } else {
                    name
                };
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

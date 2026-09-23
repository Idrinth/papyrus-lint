//! Hand-written lexer for Papyrus source.

use super::token::{IntFormat, Keyword, Token, TokenKind};
use crate::comment_annotations::parse_line_annotations;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    pending: VecDeque<Token>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            source: source.strip_prefix('\u{feff}').unwrap_or(source).as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            pending: VecDeque::new(),
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = matches!(tok.kind, TokenKind::Eof);
            // Collapse consecutive newlines / leading newlines so the
            // parser only ever has to deal with a single separator token.
            let is_newline = matches!(tok.kind, TokenKind::Newline);
            let follows_newline_or_start = matches!(
                tokens.last().map(|t: &Token| &t.kind),
                Some(TokenKind::Newline) | Option::None
            );
            if is_newline && follows_newline_or_start {
                continue;
            }
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn starts_line_continuation(&self) -> bool {
        let mut offset = 1;
        while matches!(self.peek_at(offset), Some(b' ' | b'\t')) {
            offset += 1;
        }
        matches!(self.peek_at(offset), Some(b'\n' | b'\r'))
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        if let Some(token) = self.pending.pop_front() {
            return Ok(token);
        }
        if let Some(token) = self.skip_ignorable()? {
            return Ok(token);
        }

        let line = self.line;
        let col = self.col;
        let c = self.advance().unwrap();
        match c {
            b'"' => self.read_string(line, col),
            b'0'..=b'9' => self.read_number(c, line, col),
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => self.read_word(c, line, col),
            c => Ok(Token::new(self.lex_kind(c, line, col)?, line, col)),
        }
    }

    /// Skips spaces, line continuations, and comments. Returns a token when
    /// the next significant input is EOF or a newline (both are real tokens,
    /// not ignorable).
    fn skip_ignorable(&mut self) -> Result<Option<Token>, LexError> {
        loop {
            match self.peek() {
                None => return Ok(Some(Token::new(TokenKind::Eof, self.line, self.col))),
                Some(b' ') | Some(b'\t') | Some(b'\r') => {
                    self.advance();
                }
                Some(b'\\') if self.starts_line_continuation() => {
                    // Line continuation: swallow the backslash, trailing horizontal
                    // whitespace, and the newline.
                    self.advance();
                    while matches!(self.peek(), Some(b' ' | b'\t')) {
                        self.advance();
                    }
                    if self.peek() == Some(b'\r') {
                        self.advance();
                    }
                    if self.peek() == Some(b'\n') {
                        self.advance();
                    }
                }
                Some(b'\n') => {
                    let line = self.line;
                    let col = self.col;
                    self.advance();
                    return Ok(Some(Token::new(TokenKind::Newline, line, col)));
                }
                Some(b';') => {
                    if self.peek_at(1) == Some(b'/') {
                        self.skip_block_comment()?;
                    } else {
                        self.read_line_comment_annotations();
                        if let Some(annotation) = self.pending.pop_front() {
                            return Ok(Some(annotation));
                        }
                    }
                }
                Some(b'{') => {
                    // Documentation comment blocks: `{ ... }`.
                    self.skip_brace_comment()?;
                }
                _ => return Ok(None),
            }
        }
    }

    fn lex_kind(&mut self, c: u8, line: usize, col: usize) -> Result<TokenKind, LexError> {
        match c {
            b'(' => Ok(TokenKind::LParen),
            b')' => Ok(TokenKind::RParen),
            b'[' => Ok(TokenKind::LBracket),
            b']' => Ok(TokenKind::RBracket),
            b',' => Ok(TokenKind::Comma),
            b'.' => Ok(TokenKind::Dot),
            b':' => Ok(TokenKind::Colon),
            b'+' => Ok(self.with_eq(TokenKind::PlusAssign, TokenKind::Plus)),
            b'-' => Ok(self.with_eq(TokenKind::MinusAssign, TokenKind::Minus)),
            b'*' => Ok(self.with_eq(TokenKind::StarAssign, TokenKind::Star)),
            b'/' => Ok(self.with_eq(TokenKind::SlashAssign, TokenKind::Slash)),
            b'%' => Ok(self.with_eq(TokenKind::PercentAssign, TokenKind::Percent)),
            b'=' => Ok(self.with_eq(TokenKind::Eq, TokenKind::Assign)),
            b'!' => Ok(self.with_eq(TokenKind::NotEq, TokenKind::Not)),
            b'>' => Ok(self.with_eq(TokenKind::GtEq, TokenKind::Gt)),
            b'<' => Ok(self.with_eq(TokenKind::LtEq, TokenKind::Lt)),
            b'&' if self.peek() == Some(b'&') => {
                self.advance();
                Ok(TokenKind::AndAnd)
            }
            b'|' if self.peek() == Some(b'|') => {
                self.advance();
                Ok(TokenKind::OrOr)
            }
            other => Err(LexError {
                message: format!("unexpected character '{}'", other as char),
                line,
                col,
            }),
        }
    }

    fn with_eq(&mut self, with_eq: TokenKind, without: TokenKind) -> TokenKind {
        if self.peek() == Some(b'=') {
            self.advance();
            with_eq
        } else {
            without
        }
    }

    fn read_line_comment_annotations(&mut self) {
        let start = self.pos;
        let start_col = self.col;
        while self.peek().is_some_and(|c| c != b'\n') {
            self.advance();
        }
        let comment = String::from_utf8_lossy(&self.source[start..self.pos]);
        for annotation in parse_line_annotations(&comment) {
            if matches!(
                annotation.name.to_ascii_lowercase().as_str(),
                "public" | "protected" | "private"
            ) {
                self.pending.push_back(Token::new(
                    TokenKind::CommentAnnotation(annotation.name.to_string()),
                    self.line,
                    start_col + annotation.column - 1,
                ));
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let (line, col) = (self.line, self.col);
        self.advance(); // ';'
        self.advance(); // '/'
        loop {
            match self.peek() {
                None => {
                    return Err(LexError {
                        message: "unterminated block comment".to_string(),
                        line,
                        col,
                    })
                }
                Some(b'/') if self.peek_at(1) == Some(b';') => {
                    self.advance();
                    self.advance();
                    return Ok(());
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn skip_brace_comment(&mut self) -> Result<(), LexError> {
        let (line, col) = (self.line, self.col);
        self.advance(); // '{'
        loop {
            match self.peek() {
                None => {
                    return Err(LexError {
                        message: "unterminated comment block".to_string(),
                        line,
                        col,
                    })
                }
                Some(b'}') => {
                    self.advance();
                    return Ok(());
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn read_string(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    return Err(LexError {
                        message: "unterminated string literal".to_string(),
                        line,
                        col,
                    })
                }
                Some(b'"') => {
                    self.advance();
                    break;
                }
                Some(b'\\') => {
                    self.advance();
                    let escaped = self.advance().ok_or_else(|| LexError {
                        message: "unterminated string literal".to_string(),
                        line,
                        col,
                    })?;
                    value.push(match escaped {
                        b'n' => '\n',
                        b't' => '\t',
                        b'"' => '"',
                        b'\\' => '\\',
                        other => other as char,
                    });
                }
                Some(c) => {
                    self.advance();
                    value.push(c as char);
                }
            }
        }
        Ok(Token::new(TokenKind::StringLiteral(value), line, col))
    }

    fn read_number(&mut self, first: u8, line: usize, col: usize) -> Result<Token, LexError> {
        let mut text = String::new();
        text.push(first as char);

        if first == b'0' && matches!(self.peek(), Some(b'x') | Some(b'X')) {
            text.push(self.advance().unwrap() as char);
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    text.push(self.advance().unwrap() as char);
                } else {
                    break;
                }
            }
            let value = i64::from_str_radix(&text[2..], 16).map_err(|_| LexError {
                message: format!("invalid hex literal '{}'", text),
                line,
                col,
            })?;
            return Ok(Token::new(
                TokenKind::IntLiteral(value, IntFormat::Hexadecimal),
                line,
                col,
            ));
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                text.push(self.advance().unwrap() as char);
            } else {
                break;
            }
        }

        let mut is_float = false;
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            text.push(self.advance().unwrap() as char); // '.'
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    text.push(self.advance().unwrap() as char);
                } else {
                    break;
                }
            }
        }

        if is_float {
            let value: f64 = text.parse().map_err(|_| LexError {
                message: format!("invalid float literal '{}'", text),
                line,
                col,
            })?;
            Ok(Token::new(TokenKind::FloatLiteral(value), line, col))
        } else {
            let value: i64 = text.parse().map_err(|_| LexError {
                message: format!("invalid integer literal '{}'", text),
                line,
                col,
            })?;
            Ok(Token::new(
                TokenKind::IntLiteral(value, IntFormat::Decimal),
                line,
                col,
            ))
        }
    }

    fn read_word(&mut self, first: u8, line: usize, col: usize) -> Result<Token, LexError> {
        let mut text = String::new();
        text.push(first as char);
        while let Some(c) = self.peek() {
            if c == b'_' || c.is_ascii_alphanumeric() {
                text.push(self.advance().unwrap() as char);
            } else {
                break;
            }
        }

        let lower = text.to_ascii_lowercase();
        let kind = match Keyword::from_word(&lower) {
            Some(kw) => TokenKind::Keyword(kw),
            None => TokenKind::Identifier(text),
        };
        Ok(Token::new(kind, line, col))
    }
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;

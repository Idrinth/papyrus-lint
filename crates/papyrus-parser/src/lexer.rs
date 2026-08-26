//! Hand-written lexer for Papyrus source.

use super::token::{Keyword, Token, TokenKind};

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
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            source: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
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

    fn next_token(&mut self) -> Result<Token, LexError> {
        loop {
            match self.peek() {
                None => return Ok(Token::new(TokenKind::Eof, self.line, self.col)),
                Some(b' ') | Some(b'\t') | Some(b'\r') => {
                    self.advance();
                }
                Some(b'\\') if matches!(self.peek_at(1), Some(b'\n') | Some(b'\r')) => {
                    // Line continuation: swallow the backslash and the newline.
                    self.advance();
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
                    return Ok(Token::new(TokenKind::Newline, line, col));
                }
                Some(b';') => {
                    if self.peek_at(1) == Some(b'/') {
                        self.skip_block_comment()?;
                    } else {
                        self.skip_line_comment();
                    }
                }
                Some(b'{') => {
                    // Documentation comment blocks: `{ ... }`.
                    self.skip_brace_comment()?;
                }
                _ => break,
            }
        }

        let line = self.line;
        let col = self.col;
        let c = self.advance().unwrap();

        let kind = match c {
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b'.' => TokenKind::Dot,
            b':' => TokenKind::Colon,
            b'+' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::PlusAssign
                } else {
                    TokenKind::Plus
                }
            }
            b'-' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::MinusAssign
                } else {
                    TokenKind::Minus
                }
            }
            b'*' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::StarAssign
                } else {
                    TokenKind::Star
                }
            }
            b'/' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::SlashAssign
                } else {
                    TokenKind::Slash
                }
            }
            b'%' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::PercentAssign
                } else {
                    TokenKind::Percent
                }
            }
            b'=' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::Eq
                } else {
                    TokenKind::Assign
                }
            }
            b'!' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::NotEq
                } else {
                    TokenKind::Not
                }
            }
            b'>' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            b'<' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            b'&' if self.peek() == Some(b'&') => {
                self.advance();
                TokenKind::AndAnd
            }
            b'|' if self.peek() == Some(b'|') => {
                self.advance();
                TokenKind::OrOr
            }
            b'"' => return self.read_string(line, col),
            b'0'..=b'9' => return self.read_number(c, line, col),
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => return self.read_word(c, line, col),
            other => {
                return Err(LexError {
                    message: format!("unexpected character '{}'", other as char),
                    line,
                    col,
                })
            }
        };

        Ok(Token::new(kind, line, col))
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek() {
            if c == b'\n' {
                break;
            }
            self.advance();
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
            return Ok(Token::new(TokenKind::IntLiteral(value), line, col));
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
            Ok(Token::new(TokenKind::IntLiteral(value), line, col))
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
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        Lexer::new(source)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn skips_whitespace_and_line_comments() {
        let toks = kinds("  ; a comment\nInt x");
        assert_eq!(
            toks,
            vec![
                TokenKind::Identifier("Int".to_string()),
                TokenKind::Identifier("x".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn recognizes_block_and_brace_comments() {
        let toks = kinds("Int ;/ block \n comment /; x {doc} = 1");
        assert_eq!(
            toks,
            vec![
                TokenKind::Identifier("Int".to_string()),
                TokenKind::Identifier("x".to_string()),
                TokenKind::Assign,
                TokenKind::IntLiteral(1),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn line_continuation_suppresses_newline() {
        let toks = kinds("Int x = 1 + \\\n2");
        assert_eq!(
            toks,
            vec![
                TokenKind::Identifier("Int".to_string()),
                TokenKind::Identifier("x".to_string()),
                TokenKind::Assign,
                TokenKind::IntLiteral(1),
                TokenKind::Plus,
                TokenKind::IntLiteral(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn keywords_are_case_insensitive() {
        let toks = kinds("SCRIPTNAME scriptName ScriptName");
        assert_eq!(
            toks,
            vec![
                TokenKind::Keyword(Keyword::ScriptName),
                TokenKind::Keyword(Keyword::ScriptName),
                TokenKind::Keyword(Keyword::ScriptName),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reads_numbers_and_strings() {
        let toks = kinds("1 2.5 0x1F \"hi\\nthere\"");
        assert_eq!(
            toks,
            vec![
                TokenKind::IntLiteral(1),
                TokenKind::FloatLiteral(2.5),
                TokenKind::IntLiteral(31),
                TokenKind::StringLiteral("hi\nthere".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reads_operators() {
        let toks = kinds("== != >= <= && || += -= *= /= %=");
        assert_eq!(
            toks,
            vec![
                TokenKind::Eq,
                TokenKind::NotEq,
                TokenKind::GtEq,
                TokenKind::LtEq,
                TokenKind::AndAnd,
                TokenKind::OrOr,
                TokenKind::PlusAssign,
                TokenKind::MinusAssign,
                TokenKind::StarAssign,
                TokenKind::SlashAssign,
                TokenKind::PercentAssign,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn collapses_consecutive_newlines() {
        let toks = kinds("\n\n\nInt x\n\n\nInt y");
        assert_eq!(
            toks,
            vec![
                TokenKind::Identifier("Int".to_string()),
                TokenKind::Identifier("x".to_string()),
                TokenKind::Newline,
                TokenKind::Identifier("Int".to_string()),
                TokenKind::Identifier("y".to_string()),
                TokenKind::Eof,
            ]
        );
    }
}

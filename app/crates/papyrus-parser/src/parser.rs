//! Recursive-descent parser producing an AST from a token stream.

mod declarations;
mod expressions;
mod statements;

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

/// Which game's Papyrus dialect a [`Parser`] accepts. Skyrim is the
/// original language `papyrus-parser` was built for; Fallout 4 adds a
/// handful of new constructs (custom `Struct`s, property `Group`s, the
/// `DebugOnly`/`BetaOnly` script and function flags, and colon-qualified names such
/// as `DLC03:Foo` on types, `extends`, `new`, and calls) on top of it.
/// Starfield keeps that Fallout 4 dialect and adds further flags
/// (`Private` / `Protected` / `SelfOnly` on function headers). A construct
/// that a later edition added is rejected the same way an unrecognized
/// token always is -- as an ordinary [`ParseError`] -- when parsed in a
/// mode that does not include it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameEdition {
    #[default]
    Skyrim,
    Fallout4,
    /// Starfield's Papyrus: Fallout 4's dialect plus Starfield-only
    /// header flags (`Private`, `Protected`, `SelfOnly`, `Internal`).
    Starfield,
}

impl GameEdition {
    /// Whether this edition includes Fallout 4's language extensions
    /// (`Struct`/`Group`, colon-qualified names, `is`, `New <Struct>`,
    /// `DebugOnly`/`BetaOnly`, `Const`/`Mandatory`/`Default`).
    /// [`Self::Starfield`] is a continuation of that dialect.
    pub fn has_fallout4_dialect(self) -> bool {
        matches!(self, Self::Fallout4 | Self::Starfield)
    }

    /// Whether this edition includes Starfield-only constructs such as
    /// `Guard` declarations and header access flags written as identifiers.
    pub fn has_starfield_dialect(self) -> bool {
        matches!(self, Self::Starfield)
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    mode: GameEdition,
}

impl Parser {
    /// Creates a parser over a non-empty token stream, accepting Skyrim's
    /// Papyrus dialect. See [`Self::new_with_mode`] to parse Fallout 4's
    /// or Starfield's.
    ///
    /// # Panics
    ///
    /// Panics when `tokens` is empty. Lexer-produced streams always contain
    /// at least the final [`TokenKind::Eof`] token.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self::new_with_mode(tokens, GameEdition::default())
    }

    /// Same as [`Self::new`], but accepting `mode`'s Papyrus dialect.
    ///
    /// # Panics
    ///
    /// Panics when `tokens` is empty. Lexer-produced streams always contain
    /// at least the final [`TokenKind::Eof`] token.
    pub fn new_with_mode(tokens: Vec<Token>, mode: GameEdition) -> Self {
        assert!(!tokens.is_empty(), "parser token stream must not be empty");
        Parser {
            tokens,
            pos: 0,
            mode,
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn peek_kind(&self, offset: usize) -> &TokenKind {
        let idx = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
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
        loop {
            if matches!(self.kind(), TokenKind::Newline) {
                self.advance();
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                // `@public` / `@protected` / `@private` are tokens so a
                // declaration header can record them. On a comment line of
                // their own they are not syntax; rejecting them used to
                // discard the whole script (#1180).
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Consumes a single statement terminator (newline or end of file).
    ///
    /// Access-level annotations that ride along on the same line but were
    /// not consumed as a function or property flag are comments, not code.
    fn expect_terminator(&mut self) -> PResult<()> {
        while matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
            self.advance();
        }
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

    fn at_identifier_ignore_ascii_case(&self, expected: &str) -> bool {
        matches!(self.kind(), TokenKind::Identifier(name) if name.eq_ignore_ascii_case(expected))
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

    /// Parses an identifier used for a value, including declaration flags
    /// that the Skyrim compiler permits as parameter names.
    fn expect_value_identifier(&mut self) -> PResult<String> {
        let name = match self.kind() {
            TokenKind::Keyword(Keyword::Hidden) => "hidden",
            TokenKind::Keyword(Keyword::Conditional) => "conditional",
            _ => return self.expect_identifier(),
        };
        self.advance();
        Ok(name.to_string())
    }

    /// Appends any immediately following `:Segment` pieces onto `name`.
    ///
    /// Fallout 4 uses colon-qualified names (`DLC03:Foo`,
    /// `InstanceData:Owner`, including further segments) for scripts,
    /// types, `new`, and function names. The colon is not an operator.
    fn append_colon_segments(&mut self, mut name: String) -> PResult<String> {
        while matches!(self.kind(), TokenKind::Colon) {
            self.advance();
            name.push(':');
            name.push_str(&self.expect_identifier()?);
        }
        Ok(name)
    }

    /// Parses a script name. Fallout 4 allows colon-qualified names such
    /// as `User:MyQuestScript`; Skyrim script names stay a single
    /// identifier, so a `:` is still unexpected there.
    fn expect_script_name(&mut self) -> PResult<String> {
        self.expect_qualified_name()
    }

    /// An identifier, or in Fallout 4 / Starfield a colon-qualified name
    /// (`Namespace:Name`, including further segments).
    fn expect_qualified_name(&mut self) -> PResult<String> {
        let name = self.expect_identifier()?;
        if self.mode.has_fallout4_dialect() {
            self.append_colon_segments(name)
        } else {
            Ok(name)
        }
    }

    /// Like `expect_identifier`, but also accepts the `Length` keyword,
    /// which is only ever meaningful as an array's `.Length` property.
    fn expect_property_name(&mut self) -> PResult<String> {
        if matches!(self.kind(), TokenKind::Keyword(Keyword::Length)) {
            self.advance();
            return Ok("Length".to_string());
        }
        self.expect_identifier()
    }

    /// Fallout 4 struct member names. Creation Kit accepts words the
    /// lexer treats as keywords (`parent`, `self`) as field names —
    /// F4SE's `ObjectReference.ConnectPoint` ships `string parent`.
    /// Only used from [`Self::parse_struct`], which is Fallout 4 only.
    fn expect_struct_member_name(&mut self) -> PResult<String> {
        if matches!(self.kind(), TokenKind::Keyword(Keyword::Parent)) {
            self.advance();
            return Ok("parent".to_string());
        }
        if matches!(self.kind(), TokenKind::Keyword(Keyword::Self_)) {
            self.advance();
            return Ok("self".to_string());
        }
        self.expect_identifier()
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
        if *self.kind() == kind {
            Ok(self.advance())
        } else {
            Err(self.error(format!("expected {:?}, found {:?}", kind, self.kind())))
        }
    }
}

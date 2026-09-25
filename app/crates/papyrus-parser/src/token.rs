//! Token definitions for the Papyrus lexer.
//!
//! Papyrus keywords and identifiers are case-insensitive, so keyword
//! matching happens on a lowercased copy of the source text (see `lexer.rs`).

use serde::{Deserialize, Serialize};

/// How an integer literal was written in source: plain decimal digits, or a
/// `0x`/`0X`-prefixed hexadecimal sequence. The lexer is the only place that
/// still sees the original spelling, so it records this alongside the
/// literal's parsed value; [`crate::ast::Literal::Int`] carries the same
/// distinction through into the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntFormat {
    Decimal,
    Hexadecimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keyword {
    ScriptName,
    Extends,
    Hidden,
    Conditional,
    Import,
    Function,
    EndFunction,
    Event,
    EndEvent,
    Property,
    EndProperty,
    Auto,
    AutoReadOnly,
    Global,
    Native,
    Return,
    If,
    ElseIf,
    Else,
    EndIf,
    While,
    EndWhile,
    /// Starfield only: `LockGuard <Name>[, <Name>...]` or `LockGuard(<Name>[, <Name>...])` .. `EndLockGuard`.
    LockGuard,
    EndLockGuard,
    /// Starfield only: `TryLockGuard <Name>[, <Name>...]` or `TryLockGuard(<Name>[, <Name>...])` .. `ElseTryLockGuard` .. `EndTryLockGuard`.
    TryLockGuard,
    ElseTryLockGuard,
    EndTryLockGuard,
    State,
    EndState,
    New,
    As,
    /// Fallout 4 only: type-check operator (`expr is Type`).
    Is,
    True,
    False,
    None,
    Self_,
    Parent,
    Length,
    DebugOnly,
    BetaOnly,
    /// Fallout 4 only: introduces a custom `Struct .. EndStruct` type
    /// declaration. See [`crate::parser::GameEdition::Fallout4`].
    Struct,
    EndStruct,
    /// Fallout 4 only: introduces a `Group .. EndGroup` block that wraps
    /// one or more property declarations for Creation Kit organization.
    Group,
    EndGroup,
    /// Fallout 4 only: `Group` flags controlling the group's default
    /// collapsed state in the Creation Kit's property list.
    Collapsed,
    CollapsedOnBase,
    CollapsedOnRef,
}

impl Keyword {
    /// Attempt to map a lowercased word to a keyword.
    pub fn from_word(word_lower: &str) -> Option<Keyword> {
        use Keyword::*;
        Some(match word_lower {
            "scriptname" => ScriptName,
            "extends" => Extends,
            "hidden" => Hidden,
            "conditional" => Conditional,
            "import" => Import,
            "function" => Function,
            "endfunction" => EndFunction,
            "event" => Event,
            "endevent" => EndEvent,
            "property" => Property,
            "endproperty" => EndProperty,
            "auto" => Auto,
            "autoreadonly" => AutoReadOnly,
            "global" => Global,
            "native" => Native,
            "return" => Return,
            "if" => If,
            "elseif" => ElseIf,
            "else" => Else,
            "endif" => EndIf,
            "while" => While,
            "endwhile" => EndWhile,
            "lockguard" => LockGuard,
            "endlockguard" => EndLockGuard,
            "trylockguard" => TryLockGuard,
            "elsetrylockguard" => ElseTryLockGuard,
            "endtrylockguard" => EndTryLockGuard,
            "state" => State,
            "endstate" => EndState,
            "new" => New,
            "as" => As,
            "is" => Is,
            "true" => True,
            "false" => False,
            "none" => None,
            "self" => Self_,
            "parent" => Parent,
            "length" => Length,
            "debugonly" => DebugOnly,
            "betaonly" => BetaOnly,
            "struct" => Struct,
            "endstruct" => EndStruct,
            "group" => Group,
            "endgroup" => EndGroup,
            "collapsed" => Collapsed,
            "collapsedonbase" => CollapsedOnBase,
            "collapsedonref" => CollapsedOnRef,
            _ => return Option::None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),
    IntLiteral(i64, IntFormat),
    FloatLiteral(f64),
    StringLiteral(String),
    /// A parser-relevant `@name` marker found inside a line comment.
    CommentAnnotation(String),

    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Colon,

    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,

    AndAnd,
    OrOr,
    Not,

    /// Statements in Papyrus are newline-terminated. A trailing `\` at the
    /// end of a physical line suppresses the newline token (see lexer).
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token { kind, line, col }
    }
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;

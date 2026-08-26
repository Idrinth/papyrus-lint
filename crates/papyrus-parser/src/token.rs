//! Token definitions for the Papyrus lexer.
//!
//! Papyrus keywords and identifiers are case-insensitive, so keyword
//! matching happens on a lowercased copy of the source text (see `lexer.rs`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    State,
    EndState,
    New,
    As,
    True,
    False,
    None,
    Self_,
    Parent,
    Length,
    DebugOnly,
    BetaOnly,
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
            "state" => State,
            "endstate" => EndState,
            "new" => New,
            "as" => As,
            "true" => True,
            "false" => False,
            "none" => None,
            "self" => Self_,
            "parent" => Parent,
            "length" => Length,
            "debugonly" => DebugOnly,
            "betaonly" => BetaOnly,
            _ => return Option::None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),

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

#[derive(Debug, Clone, PartialEq)]
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

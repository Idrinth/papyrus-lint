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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),
    IntLiteral(i64, IntFormat),
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
mod tests {
    use super::*;

    #[test]
    fn maps_every_keyword_spelling() {
        use Keyword::*;

        let cases = [
            ("scriptname", ScriptName),
            ("extends", Extends),
            ("hidden", Hidden),
            ("conditional", Conditional),
            ("import", Import),
            ("function", Function),
            ("endfunction", EndFunction),
            ("event", Event),
            ("endevent", EndEvent),
            ("property", Property),
            ("endproperty", EndProperty),
            ("auto", Auto),
            ("autoreadonly", AutoReadOnly),
            ("global", Global),
            ("native", Native),
            ("return", Return),
            ("if", If),
            ("elseif", ElseIf),
            ("else", Else),
            ("endif", EndIf),
            ("while", While),
            ("endwhile", EndWhile),
            ("state", State),
            ("endstate", EndState),
            ("new", New),
            ("as", As),
            ("true", True),
            ("false", False),
            ("none", None),
            ("self", Self_),
            ("parent", Parent),
            ("length", Length),
            ("debugonly", DebugOnly),
            ("betaonly", BetaOnly),
        ];

        for (spelling, expected) in cases {
            assert_eq!(Keyword::from_word(spelling), Some(expected), "{spelling}");
        }
    }

    #[test]
    fn rejects_non_keywords_and_non_normalized_case() {
        assert_eq!(Keyword::from_word("identifier"), None);
        assert_eq!(Keyword::from_word("Function"), None);
        assert_eq!(Keyword::from_word(""), None);
    }

    #[test]
    fn constructs_token_with_source_position() {
        let token = Token::new(TokenKind::Identifier("value".to_string()), 12, 7);

        assert_eq!(token.kind, TokenKind::Identifier("value".to_string()));
        assert_eq!(token.line, 12);
        assert_eq!(token.col, 7);
    }

    /// Tokens need to round-trip through serde so a disk-backed cache can
    /// persist them alongside the parsed AST, rather than only ever holding
    /// them in memory for the duration of a single lint pass.
    #[test]
    fn token_round_trips_through_json_including_a_keyword_and_hex_int_literal() {
        let tokens = vec![
            Token::new(TokenKind::Keyword(Keyword::ScriptName), 1, 1),
            Token::new(TokenKind::IntLiteral(42, IntFormat::Hexadecimal), 2, 5),
            Token::new(TokenKind::Eof, 3, 1),
        ];

        let json = serde_json::to_string(&tokens).unwrap();
        let round_tripped: Vec<Token> = serde_json::from_str(&json).unwrap();

        assert_eq!(round_tripped, tokens);
    }
}

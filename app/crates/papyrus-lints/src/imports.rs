//! Shared `Import` helpers.
//!
//! Papyrus's `Import ScriptName` statement makes that script's `Global`
//! functions callable without a qualifier. `Utility.Wait(0.2)` and
//! `Import Utility` plus `Wait(0.2)` are the same call; the same is true
//! of `Game.GetPlayer()` and `Import Game` plus `GetPlayer()`.

use papyrus_parser::ast::Script;
use papyrus_parser::token::{Keyword, Token, TokenKind};

/// Whether `script` contains `Import name` (identifiers are case-insensitive).
pub(crate) fn script_imports(script: Option<&Script>, name: &str) -> bool {
    script
        .map(|script| {
            script
                .imports
                .iter()
                .any(|import| import.name.eq_ignore_ascii_case(name))
        })
        .unwrap_or(false)
}

/// Whether the token stream contains `Import name`.
pub(crate) fn tokens_import(tokens: &[Token], name: &str) -> bool {
    tokens.windows(2).any(|window| {
        matches!(window[0].kind, TokenKind::Keyword(Keyword::Import))
            && matches!(&window[1].kind, TokenKind::Identifier(imported) if imported.eq_ignore_ascii_case(name))
    })
}

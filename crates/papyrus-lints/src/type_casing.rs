//! Enforces a configured casing convention on the type a script declares,
//! i.e. the name following its `ScriptName` statement.
//!
//! Works on lexer tokens (rather than the parsed AST) so it still runs on
//! scripts that don't parse cleanly, matching every other token-based lint
//! in this crate. Only the script's own declared name is checked — a
//! script's `Extends` target is a type declared (and presumably already
//! checked) elsewhere, so flagging it here would just repeat that other
//! script's diagnostic under the wrong file.

use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{Keyword, TokenKind};
use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "type-casing";

/// The supported casing conventions for a script's declared type name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Style {
    /// `MyQuestScript`: starts with an uppercase letter, no underscores.
    #[default]
    #[serde(rename = "PascalCase")]
    PascalCase,
    /// `myQuestScript`: starts with a lowercase letter, no underscores.
    #[serde(rename = "camelCase")]
    CamelCase,
    /// `myquestscript`: no uppercase letters anywhere.
    #[serde(rename = "lowercase")]
    Lowercase,
    /// `MYQUESTSCRIPT`: no lowercase letters anywhere.
    #[serde(rename = "UPPERCASE")]
    Uppercase,
}

impl Style {
    /// The human-readable label used in this lint's diagnostic message,
    /// matching the YAML value that selects it.
    fn label(self) -> &'static str {
        match self {
            Style::PascalCase => "PascalCase",
            Style::CamelCase => "camelCase",
            Style::Lowercase => "lowercase",
            Style::Uppercase => "UPPERCASE",
        }
    }

    /// Whether `name` conforms to this casing convention.
    fn matches(self, name: &str) -> bool {
        match self {
            Style::PascalCase => !name.contains('_') && first_letter_case(name) != Some(false),
            Style::CamelCase => !name.contains('_') && first_letter_case(name) != Some(true),
            Style::Lowercase => name
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_lowercase),
            Style::Uppercase => name
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase),
        }
    }
}

/// `Some(true)`/`Some(false)` for the name's first alphabetic character
/// being upper-/lowercase, or `None` if it has no alphabetic character at
/// all (in which case casing doesn't apply, so callers treat that as a
/// match).
fn first_letter_case(name: &str) -> Option<bool> {
    name.chars()
        .find(|c| c.is_alphabetic())
        .map(char::is_uppercase)
}

/// Checks `source`'s declared `ScriptName` against `style`. A script with
/// no `ScriptName` statement, or one that fails to lex, yields no
/// diagnostics.
pub fn check(source: &str, style: Style) -> Vec<Diagnostic> {
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return Vec::new();
    };

    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        if token.kind != TokenKind::Keyword(Keyword::ScriptName) {
            continue;
        }
        let Some(name_token) = tokens.next() else {
            return Vec::new();
        };
        let TokenKind::Identifier(name) = &name_token.kind else {
            return Vec::new();
        };
        if style.matches(name) {
            return Vec::new();
        }
        return vec![Diagnostic {
            line: name_token.line,
            column: name_token.col,
            message: format!(
                "[warning] Script name '{name}' does not follow the configured {} casing",
                style.label()
            ),
            rule: RULE,
        }];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_accepts_a_conforming_name() {
        assert!(check("ScriptName MyQuestScript\n", Style::PascalCase).is_empty());
    }

    #[test]
    fn pascal_case_flags_a_lowercase_start() {
        let diagnostics = check("ScriptName myQuestScript\n", Style::PascalCase);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("myQuestScript"));
        assert!(diagnostics[0].message.contains("PascalCase"));
    }

    #[test]
    fn pascal_case_flags_underscores() {
        assert_eq!(
            check("ScriptName My_QuestScript\n", Style::PascalCase).len(),
            1
        );
    }

    #[test]
    fn camel_case_accepts_a_conforming_name() {
        assert!(check("ScriptName myQuestScript\n", Style::CamelCase).is_empty());
    }

    #[test]
    fn camel_case_flags_an_uppercase_start() {
        assert_eq!(
            check("ScriptName MyQuestScript\n", Style::CamelCase).len(),
            1
        );
    }

    #[test]
    fn lowercase_accepts_an_all_lowercase_name() {
        assert!(check("ScriptName myquestscript\n", Style::Lowercase).is_empty());
    }

    #[test]
    fn lowercase_flags_any_uppercase_letter() {
        assert_eq!(
            check("ScriptName myQuestscript\n", Style::Lowercase).len(),
            1
        );
    }

    #[test]
    fn uppercase_accepts_an_all_uppercase_name() {
        assert!(check("ScriptName MYQUESTSCRIPT\n", Style::Uppercase).is_empty());
    }

    #[test]
    fn uppercase_flags_any_lowercase_letter() {
        assert_eq!(
            check("ScriptName MyQUESTSCRIPT\n", Style::Uppercase).len(),
            1
        );
    }

    #[test]
    fn reports_the_declared_name_position_not_the_keyword() {
        let diagnostics = check(
            "Scriptname   myQuestScript Extends Quest\n",
            Style::PascalCase,
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 14);
    }

    #[test]
    fn script_with_no_scriptname_statement_is_unflagged() {
        assert!(check("Function Foo()\nEndFunction\n", Style::PascalCase).is_empty());
    }

    #[test]
    fn a_script_that_fails_to_lex_is_left_unchecked() {
        assert!(check("ScriptName Example \"unterminated\n", Style::PascalCase).is_empty());
    }

    #[test]
    fn extends_target_casing_is_never_checked() {
        // Only the declared name (Example) is checked, not the Extends
        // target (badlyCasedParent) which belongs to another script.
        assert!(check(
            "ScriptName Example Extends badlyCasedParent\n",
            Style::PascalCase
        )
        .is_empty());
    }
}

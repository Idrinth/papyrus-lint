//! Enforces a configured casing convention on the type a script declares,
//! i.e. the name following its `ScriptName` statement.
//!
//! Works on lexer tokens (rather than the parsed AST) so it still runs on
//! scripts that don't parse cleanly, matching every other token-based lint
//! in this crate. Only the script's own declared name is checked — a
//! script's `Extends` target is a type declared (and presumably already
//! checked) elsewhere, so flagging it here would just repeat that other
//! script's diagnostic under the wrong file. A leading acronym prefix
//! (`IDR__TIF__050000F5`, `USSEP_MyQuestScript`) that CreationKit or modding
//! convention enforces is stripped before checking casing (see
//! [`strip_known_prefix`]), since that part of the name can't be renamed at
//! all.

use papyrus_parser::token::{Keyword, TokenKind};
use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "type-casing";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::tokens()
}

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

    /// Whether repairing `name` to this style is possible through a
    /// letter-casing change alone (see [`Style::apply`]), i.e. without a
    /// substantive rename (such as removing an underscore for `PascalCase`)
    /// that would break the required filename/`ScriptName` match.
    fn fixable(self, name: &str) -> bool {
        let replacement = self.apply(name);
        replacement != *name && self.matches(&replacement)
    }

    /// Returns the case-only rewrite of `name` for this convention.
    fn apply(self, name: &str) -> String {
        match self {
            Style::Lowercase => name.to_lowercase(),
            Style::Uppercase => name.to_uppercase(),
            Style::PascalCase | Style::CamelCase => {
                let mut result = String::with_capacity(name.len());
                let mut first_letter = true;

                for character in name.chars() {
                    if character.is_alphabetic() && first_letter {
                        if self == Style::PascalCase {
                            result.extend(character.to_uppercase());
                        } else {
                            result.extend(character.to_lowercase());
                        }
                        first_letter = false;
                    } else {
                        result.push(character);
                    }
                }

                result
            }
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

/// Strips up to two leading acronym-prefix segments — one or more uppercase
/// letters immediately followed by one or more underscores — from `name`,
/// returning whatever remains. CreationKit itself names a dialogue
/// fragment's script this way (e.g. `IDR__TIF__050000F5`, the quest's own
/// editor ID followed by a literal `TIF` marker), and modders conventionally
/// prefix their own scripts with an uppercase mod acronym for the same
/// reason (e.g. `USSEP_MyQuestScript`) — in both cases the prefix can't be
/// renamed without breaking the naming scheme it belongs to, so casing is
/// checked (and repaired) against the rest of the name instead. Returns
/// `name` itself unchanged if stripping would consume the whole thing, or if
/// it carries no such prefix at all.
fn strip_known_prefix(name: &str) -> &str {
    let mut rest = name;
    for _ in 0..2 {
        let letters_end = rest
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_uppercase())
            .last()
            .map_or(0, |(index, c)| index + c.len_utf8());
        if letters_end == 0 {
            break;
        }
        let underscore_end = rest[letters_end..]
            .char_indices()
            .take_while(|(_, c)| *c == '_')
            .last()
            .map_or(letters_end, |(index, _)| letters_end + index + 1);
        if underscore_end == letters_end {
            break;
        }
        rest = &rest[underscore_end..];
    }
    if rest.is_empty() {
        name
    } else {
        rest
    }
}

/// Checks `source`'s declared `ScriptName` against `style`. A script with
/// no `ScriptName` statement, or one that fails to lex, yields no
/// diagnostics. Casing is checked against the name with any leading
/// CreationKit/mod-acronym prefix stripped (see [`strip_known_prefix`]), so
/// such a prefix's own casing never triggers a violation. A violation that
/// [`repair`] can't actually fix (e.g. a name with underscores under
/// `PascalCase`/`camelCase`) says so in its own message, so callers don't
/// present it as automatically fixable when it isn't.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, ast, external);
    let style = config.type_casing;

    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let mut tokens = tokens.iter().cloned();
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
        let checked = strip_known_prefix(name);
        if style.matches(checked) {
            return Vec::new();
        }
        // A violation this rule can't actually repair (see `Style::fixable`)
        // says so in its own message, since the frontend otherwise has no
        // way to tell such a finding apart from one this rule's automatic
        // fix can resolve.
        let unfixable_note = if style.fixable(checked) {
            ""
        } else {
            " (fixing this would rename the script, so no automatic fix is applied)"
        };
        return vec![Diagnostic {
            line: name_token.line,
            column: name_token.col,
            message: format!(
                "[warning] Script name '{name}' does not follow the configured {} casing{unfixable_note}",
                style.label()
            ),
            rule: RULE,
        }];
    }
    Vec::new()
}

/// Rewrites the declared `ScriptName` identifier to match `style` when letter
/// casing alone can do so. Only the declaration is changed; references to
/// other types (including the `Extends` target) are left untouched. Any
/// leading CreationKit/mod-acronym prefix (see [`strip_known_prefix`]) is
/// left as-is; only the remainder of the name is rewritten. A repair that
/// would add or remove characters is skipped so the declaration remains
/// compatible with its filename. Invalid source is returned verbatim.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let style = config.type_casing;

    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };

    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        if token.kind != TokenKind::Keyword(Keyword::ScriptName) {
            continue;
        }
        let Some(name_token) = tokens.next() else {
            return source.to_string();
        };
        let TokenKind::Identifier(name) = &name_token.kind else {
            return source.to_string();
        };
        let checked = strip_known_prefix(name);
        // PascalCase/camelCase also prohibit underscores, but removing one
        // would be a substantive rename and break the required
        // filename/ScriptName match. Only apply a repair when changing case
        // alone can make the declaration conform.
        if !style.fixable(checked) {
            return source.to_string();
        }
        let prefix_len = name.len() - checked.len();
        let replacement = format!("{}{}", &name[..prefix_len], style.apply(checked));

        let line_start = if name_token.line == 1 {
            0
        } else {
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1))
                .nth(name_token.line - 2)
                .unwrap_or(0)
        };
        let start = line_start + name_token.col - 1;
        let end = start + name.len();
        let mut repaired = source.to_string();
        repaired.replace_range(start..end, &replacement);
        return repaired;
    }

    source.to_string()
}

#[cfg(test)]
#[path = "type_casing_tests.rs"]
mod tests;

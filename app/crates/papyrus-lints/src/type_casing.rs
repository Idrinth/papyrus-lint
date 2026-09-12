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
pub fn check(source: &str, style: Style) -> Vec<Diagnostic> {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
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
pub fn repair(source: &str, style: Style) -> String {
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
        // A letter-casing-only rewrite ("MyQuestScript") fixes this one, so
        // it must not be flagged as unfixable.
        assert!(!diagnostics[0].message.contains("no automatic fix"));
    }

    #[test]
    fn pascal_case_flags_underscores() {
        let diagnostics = check("ScriptName My_QuestScript\n", Style::PascalCase);
        assert_eq!(diagnostics.len(), 1);
        // Removing the underscore would be a substantive rename `repair`
        // won't make, so the diagnostic must say so rather than implying an
        // automatic fix is available.
        assert!(diagnostics[0].message.contains("no automatic fix"));
    }

    #[test]
    fn ignores_a_creationkit_fragment_style_prefix() {
        // CreationKit itself names dialogue Topic Info fragment scripts this
        // way (`<QuestEditorID>__TIF__<FormID>`); the two leading
        // uppercase-acronym-plus-underscores segments are stripped before
        // checking casing, so the remainder (all digits/uppercase) matches
        // PascalCase trivially instead of being flagged as an unfixable
        // underscore violation.
        assert!(check("ScriptName IDR__TIF__050000F5\n", Style::PascalCase).is_empty());
    }

    #[test]
    fn ignores_a_mod_acronym_prefix() {
        // A modder's own uppercase-acronym prefix (single underscore) is
        // stripped the same way, so only `MyQuestScript` is checked.
        assert!(check("ScriptName USSEP_MyQuestScript\n", Style::PascalCase).is_empty());
    }

    #[test]
    fn does_not_treat_a_single_leading_capital_as_a_prefix() {
        // "My_QuestScript" isn't a recognized acronym prefix (the uppercase
        // run before the underscore is just the name's own first letter),
        // so this is still flagged, and still unfixable since removing the
        // underscore would be a substantive rename.
        let diagnostics = check("ScriptName My_QuestScript\n", Style::PascalCase);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("no automatic fix"));
    }

    #[test]
    fn repair_leaves_a_recognized_prefix_untouched() {
        assert_eq!(
            repair("ScriptName USSEP_myQuestScript\n", Style::PascalCase),
            "ScriptName USSEP_MyQuestScript\n"
        );
    }

    #[test]
    fn strip_known_prefix_handles_up_to_two_segments() {
        assert_eq!(strip_known_prefix("IDR__TIF__050000F5"), "050000F5");
        assert_eq!(strip_known_prefix("USSEP_MyQuestScript"), "MyQuestScript");
        assert_eq!(strip_known_prefix("MyQuestScript"), "MyQuestScript");
        assert_eq!(strip_known_prefix("My_QuestScript"), "My_QuestScript");
        // A name that's entirely a prefix (nothing left after stripping) is
        // returned unchanged rather than reduced to an empty string.
        assert_eq!(strip_known_prefix("USSEP_"), "USSEP_");
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
    fn lowercase_and_uppercase_allow_underscores_and_digits() {
        assert!(check("ScriptName quest_script2\n", Style::Lowercase).is_empty());
        assert!(check("ScriptName QUEST_SCRIPT2\n", Style::Uppercase).is_empty());
    }

    #[test]
    fn names_without_letters_do_not_have_a_case_violation() {
        assert!(check("ScriptName _123\n", Style::Lowercase).is_empty());
        assert!(check("ScriptName _123\n", Style::Uppercase).is_empty());

        // PascalCase and camelCase still reject this identifier because
        // those conventions independently prohibit underscores.
        assert_eq!(check("ScriptName _123\n", Style::PascalCase).len(), 1);
        assert_eq!(check("ScriptName _123\n", Style::CamelCase).len(), 1);
    }

    #[test]
    fn every_style_uses_its_config_value_in_the_diagnostic() {
        for (style, name, label) in [
            (Style::PascalCase, "example", "PascalCase"),
            (Style::CamelCase, "Example", "camelCase"),
            (Style::Lowercase, "ExAmPlE", "lowercase"),
            (Style::Uppercase, "ExAmPlE", "UPPERCASE"),
        ] {
            let diagnostics = check(&format!("ScriptName {name}\n"), style);
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains(label));
        }
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
    fn incomplete_or_non_identifier_declarations_are_unflagged() {
        assert!(check("ScriptName", Style::PascalCase).is_empty());
        assert!(check("ScriptName Int\n", Style::PascalCase).is_empty());
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

    #[test]
    fn repair_applies_each_configured_style() {
        assert_eq!(
            repair("ScriptName myQuestScript\n", Style::PascalCase),
            "ScriptName MyQuestScript\n"
        );
        assert_eq!(
            repair("ScriptName MyQuestScript\n", Style::CamelCase),
            "ScriptName myQuestScript\n"
        );
        assert_eq!(
            repair("ScriptName MyQuestScript\n", Style::Lowercase),
            "ScriptName myquestscript\n"
        );
        assert_eq!(
            repair("ScriptName MyQuestScript\n", Style::Uppercase),
            "ScriptName MYQUESTSCRIPT\n"
        );
    }

    #[test]
    fn repair_changes_only_the_declared_name_and_is_idempotent() {
        let source = "ScriptName myScript Extends badly_cased_parent\r\n";
        let repaired = repair(source, Style::PascalCase);

        assert_eq!(
            repaired,
            "ScriptName MyScript Extends badly_cased_parent\r\n"
        );
        assert_eq!(repair(&repaired, Style::PascalCase), repaired);
    }

    #[test]
    fn repair_finds_the_declaration_after_an_earlier_line() {
        let source = "; generated file\r\nScriptName myScript Extends Parent\r\n";
        assert_eq!(
            repair(source, Style::PascalCase),
            "; generated file\r\nScriptName MyScript Extends Parent\r\n"
        );
    }

    #[test]
    fn lowercase_and_uppercase_repairs_preserve_non_letters() {
        assert_eq!(
            repair("ScriptName Quest_Script2\n", Style::Lowercase),
            "ScriptName quest_script2\n"
        );
        assert_eq!(
            repair("ScriptName Quest_Script2\n", Style::Uppercase),
            "ScriptName QUEST_SCRIPT2\n"
        );
    }

    #[test]
    fn repair_does_not_make_a_substantive_rename() {
        for style in [Style::PascalCase, Style::CamelCase] {
            let source = "ScriptName my_questScript\n";
            assert_eq!(repair(source, style), source);
        }
    }

    #[test]
    fn repair_leaves_invalid_or_declaration_free_source_unchanged() {
        for source in [
            "Function Foo()\nEndFunction\n",
            "ScriptName Example \"unterminated\n",
            "ScriptName",
            "ScriptName Int\n",
        ] {
            assert_eq!(repair(source, Style::PascalCase), source);
        }
    }

    #[test]
    fn pascal_case_is_the_default_style() {
        assert_eq!(Style::default(), Style::PascalCase);
    }
}

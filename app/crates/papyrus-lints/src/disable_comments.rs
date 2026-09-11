//! Parses `@disable <rule-id>[, <rule-id>...]` and
//! `@disable-file <rule-id>[, <rule-id>...]` directives out of trailing `;`
//! line comments. `@disable` suppresses specific lints on just the line it
//! appears on, e.g.:
//!
//! ```papyrus
//! action = 1 ; @disable float-to-int
//! ```
//!
//! `@disable-file` suppresses them across the entire file instead, no
//! matter which line it's written on, e.g.:
//!
//! ```papyrus
//! ; @disable-file float-to-int
//! ```
//!
//! `; @disable`/`; @disable-file` with no rule ids suppresses every lint on
//! that line, or across the whole file, respectively. Matching is against
//! each lint's [`crate::Diagnostic::rule`] id (case-insensitive); only
//! plain `;` line comments are recognized, not `;/ ... /;` block comments
//! or `{ ... }` brace comments. This only affects
//! [`crate::lint`]/[`crate::lint_with_external_arguments`] — it has no
//! effect on [`crate::repair`].

use std::collections::{HashMap, HashSet};

/// Which rules a directive disables: every rule, or a specific set of rule
/// ids (lowercased). Shared shape for both a single-line `@disable` and a
/// whole-file `@disable-file` directive.
#[derive(Debug)]
pub(crate) enum Directive {
    All { column: usize },
    Rules(Vec<DisableRule>),
}

#[derive(Debug)]
pub(crate) struct DisableRule {
    pub(crate) id: String,
    pub(crate) column: usize,
}

/// Maps 1-indexed line numbers to the rules disabled on that line via
/// `@disable`, plus every `@disable-file` directive found anywhere in the
/// source (each paired with the line/column it was written on, for
/// [`crate::unused_disable`] to report against).
pub struct Disables {
    lines: HashMap<usize, Directive>,
    file: Vec<(usize, Directive)>,
}

impl Disables {
    /// Scans `source` for `@disable`/`@disable-file` directives in trailing
    /// line comments.
    pub fn scan(source: &str) -> Self {
        let mut lines = HashMap::new();
        let mut file = Vec::new();
        for (index, line) in source.lines().enumerate() {
            let line_number = index + 1;
            if let Some(directive) = parse_directive(line, "@disable") {
                lines.insert(line_number, directive);
            }
            if let Some(directive) = parse_directive(line, "@disable-file") {
                file.push((line_number, directive));
            }
        }
        Disables { lines, file }
    }

    /// Whether `rule` is disabled on `line` (1-indexed), either by an
    /// `@disable-file` directive anywhere in the file or an `@disable`
    /// directive on that specific line.
    pub fn is_disabled(&self, line: usize, rule: &str) -> bool {
        if self.file_disables(rule) {
            return true;
        }
        match self.lines.get(&line) {
            None => false,
            Some(Directive::All { .. }) => true,
            Some(Directive::Rules(rules)) => rules
                .iter()
                .any(|disabled| disabled.id == rule.to_ascii_lowercase()),
        }
    }

    fn file_disables(&self, rule: &str) -> bool {
        self.file.iter().any(|(_, directive)| match directive {
            Directive::All { .. } => true,
            Directive::Rules(rules) => rules
                .iter()
                .any(|disabled| disabled.id == rule.to_ascii_lowercase()),
        })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &Directive)> {
        self.lines.iter().map(|(&line, disable)| (line, disable))
    }

    pub(crate) fn file_iter(&self) -> impl Iterator<Item = (usize, &Directive)> {
        self.file.iter().map(|(line, disable)| (*line, disable))
    }
}

/// Finds a `keyword` (`@disable` or `@disable-file`) directive within
/// `line`'s trailing line comment, if it has one.
fn parse_directive(line: &str, keyword: &str) -> Option<Directive> {
    let comment = line_comment_text(line)?;
    // `to_ascii_lowercase` maps each byte to itself or another single ASCII
    // byte, so the index found still lands on the same offset in `comment`.
    let index = comment.to_ascii_lowercase().find(keyword)?;
    let after = &comment[index + keyword.len()..];
    // Require a word boundary so `@disabled-rule`/`@disable-file-thing`
    // isn't mistaken for the directive with an empty rule list.
    if after.starts_with(|c: char| !c.is_whitespace()) {
        return None;
    }

    let rest = after.trim();
    if rest.is_empty() {
        return Some(Directive::All {
            column: line[..line.len() - after.len() - keyword.len()]
                .chars()
                .count()
                + 1,
        });
    }

    let rest_offset = line.len() - rest.len();
    let mut seen = HashSet::new();
    let rules = rule_parts(rest)
        .filter_map(|(offset, rule)| {
            let id = rule.to_ascii_lowercase();
            seen.insert(id.clone()).then_some(DisableRule {
                id,
                column: line[..rest_offset + offset].chars().count() + 1,
            })
        })
        .collect();
    Some(Directive::Rules(rules))
}

fn rule_parts(value: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = None;
    value
        .char_indices()
        .chain(std::iter::once((value.len(), ',')))
        .filter_map(move |(offset, character)| {
            if character == ',' || character.is_whitespace() {
                start.take().map(|start| (start, &value[start..offset]))
            } else {
                start.get_or_insert(offset);
                None
            }
        })
}

/// Returns the text following the `;` that starts `line`'s line comment, if
/// any, ignoring semicolons inside string literals and treating a `;/`
/// block-comment opener as not starting a line comment.
fn line_comment_text(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => index += 1,
            b';' if !in_string => {
                return if bytes.get(index + 1) == Some(&b'/') {
                    None
                } else {
                    Some(&line[index + 1..])
                };
            }
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disables_a_specific_rule_on_its_line() {
        let disables = Disables::scan("action = 1 ; @disable float-to-int\nother = 2\n");
        assert!(disables.is_disabled(1, "float-to-int"));
        assert!(!disables.is_disabled(1, "strict-boolean"));
        assert!(!disables.is_disabled(2, "float-to-int"));
    }

    #[test]
    fn disables_are_case_insensitive() {
        let disables = Disables::scan("action = 1 ; @DISABLE Float-To-Int\n");
        assert!(disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn disables_multiple_comma_separated_rules() {
        let disables = Disables::scan("Foo(1,2) ; @disable comma-spacing, float-to-int\n");
        assert!(disables.is_disabled(1, "comma-spacing"));
        assert!(disables.is_disabled(1, "float-to-int"));
        assert!(!disables.is_disabled(1, "semicolon"));
    }

    #[test]
    fn accepts_mixed_comma_and_whitespace_separators() {
        let disables =
            Disables::scan("Foo(1,2) ; @disable comma-spacing  float-to-int,semicolon\n");

        assert!(disables.is_disabled(1, "comma-spacing"));
        assert!(disables.is_disabled(1, "float-to-int"));
        assert!(disables.is_disabled(1, "semicolon"));
    }

    #[test]
    fn duplicate_rule_ids_are_recorded_only_once_at_the_first_column() {
        let disables = Disables::scan("value = 1 ; @disable alpha, beta, ALPHA\n");
        let (_, disable) = disables.iter().next().expect("directive should be found");
        let Directive::Rules(rules) = disable else {
            panic!("named rules should not become an all-rules directive");
        };

        assert_eq!(rules.len(), 2);
        assert_eq!((rules[0].id.as_str(), rules[0].column), ("alpha", 22));
        assert_eq!((rules[1].id.as_str(), rules[1].column), ("beta", 29));
    }

    #[test]
    fn bare_disable_suppresses_every_rule_on_the_line() {
        let disables = Disables::scan("action = 1  ; @disable\n");
        assert!(disables.is_disabled(1, "trailing-whitespace"));
        assert!(disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn ignores_semicolons_inside_string_literals() {
        let disables = Disables::scan("Debug.Trace(\"a;b @disable float-to-int\")\n");
        assert!(!disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn escaped_quotes_do_not_end_a_string_or_expose_its_semicolon() {
        let disables = Disables::scan(r#"Debug.Trace("escaped \"; @disable float-to-int\"")"#);

        assert!(!disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn finds_a_directive_after_a_string_containing_a_semicolon() {
        let disables = Disables::scan(r#"Debug.Trace("still; a string") ; @disable semicolon"#);

        assert!(disables.is_disabled(1, "semicolon"));
    }

    #[test]
    fn reports_character_columns_for_unicode_before_a_directive() {
        let disables = Disables::scan("String text = \"λ\" ; @disable comma-spacing\n");
        let (_, disable) = disables.iter().next().expect("directive should be found");
        let Directive::Rules(rules) = disable else {
            panic!("named rules should not become an all-rules directive");
        };

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].column, 30);
    }

    #[test]
    fn ignores_disable_like_text_without_the_directive_word() {
        let disables = Disables::scan("action = 1 ; @disabled-thing float-to-int\n");
        assert!(!disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn ignores_block_comment_openers() {
        let disables = Disables::scan("action = 1 ;/ @disable float-to-int /;\n");
        assert!(!disables.is_disabled(1, "float-to-int"));
    }

    #[test]
    fn lines_without_a_directive_are_not_disabled() {
        let disables = Disables::scan("action = 1\n; just a comment\n");
        assert!(!disables.is_disabled(1, "float-to-int"));
        assert!(!disables.is_disabled(2, "float-to-int"));
    }

    #[test]
    fn disable_file_suppresses_a_specific_rule_on_every_line() {
        let disables = Disables::scan("; @disable-file float-to-int\naction = 1\nother = 2\n");
        assert!(disables.is_disabled(2, "float-to-int"));
        assert!(disables.is_disabled(3, "float-to-int"));
        assert!(!disables.is_disabled(2, "strict-boolean"));
    }

    #[test]
    fn bare_disable_file_suppresses_every_rule_on_every_line() {
        let disables = Disables::scan("; @disable-file\naction = 1\nother = 2\n");
        assert!(disables.is_disabled(2, "trailing-whitespace"));
        assert!(disables.is_disabled(3, "float-to-int"));
    }

    #[test]
    fn disable_file_need_not_be_on_the_first_line() {
        let disables = Disables::scan("action = 1\nother = 2 ; @disable-file comma-spacing\n");
        assert!(disables.is_disabled(1, "comma-spacing"));
        assert!(disables.is_disabled(2, "comma-spacing"));
    }

    #[test]
    fn disable_file_is_case_insensitive_and_supports_multiple_rules() {
        let disables = Disables::scan("; @DISABLE-FILE Comma-Spacing, Float-To-Int\naction = 1\n");
        assert!(disables.is_disabled(2, "comma-spacing"));
        assert!(disables.is_disabled(2, "float-to-int"));
    }

    #[test]
    fn disable_file_does_not_apply_to_a_plain_disable_line() {
        let disables = Disables::scan("action = 1 ; @disable float-to-int\nother = 2\n");
        assert!(!disables.is_disabled(2, "float-to-int"));
    }

    #[test]
    fn ignores_disable_file_like_text_without_the_directive_word() {
        let disables = Disables::scan("action = 1 ; @disable-filetype float-to-int\nother = 2\n");
        assert!(!disables.is_disabled(1, "float-to-int"));
        assert!(!disables.is_disabled(2, "float-to-int"));
    }

    #[test]
    fn disable_file_directive_does_not_also_register_as_a_line_disable() {
        let disables = Disables::scan("; @disable-file float-to-int\n");
        let (_, disable) = disables
            .file_iter()
            .next()
            .expect("file directive should be found");
        let Directive::Rules(rules) = disable else {
            panic!("named rules should not become an all-rules directive");
        };
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "float-to-int");
        assert!(disables.iter().next().is_none());
    }
}

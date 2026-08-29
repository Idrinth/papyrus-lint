//! Parses `@disable <rule-id>[, <rule-id>...]` directives out of trailing
//! `;` line comments, so a single line can suppress specific lints without
//! turning them off project-wide, e.g.:
//!
//! ```papyrus
//! action = 1 ; @disable float-to-int
//! ```
//!
//! `; @disable` with no rule ids suppresses every lint on that line.
//! Matching is against each lint's [`crate::Diagnostic::rule`] id
//! (case-insensitive); only plain `;` line comments are recognized, not
//! `;/ ... /;` block comments or `{ ... }` brace comments. This only
//! affects [`crate::lint`]/[`crate::lint_with_external_arguments`] — it has
//! no effect on [`crate::repair`].

use std::collections::{HashMap, HashSet};

/// Which rules are disabled on a line: every rule, or a specific set of
/// rule ids (lowercased).
enum LineDisable {
    All,
    Rules(HashSet<String>),
}

/// Maps 1-indexed line numbers to the rules disabled on that line.
pub struct Disables(HashMap<usize, LineDisable>);

impl Disables {
    /// Scans `source` for `@disable` directives in trailing line comments.
    pub fn scan(source: &str) -> Self {
        let map = source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| Some((index + 1, parse_directive(line)?)))
            .collect();
        Disables(map)
    }

    /// Whether `rule` is disabled on `line` (1-indexed).
    pub fn is_disabled(&self, line: usize, rule: &str) -> bool {
        match self.0.get(&line) {
            None => false,
            Some(LineDisable::All) => true,
            Some(LineDisable::Rules(rules)) => rules.contains(&rule.to_ascii_lowercase()),
        }
    }
}

/// Finds an `@disable` directive within `line`'s trailing line comment, if
/// it has one.
fn parse_directive(line: &str) -> Option<LineDisable> {
    let comment = line_comment_text(line)?;
    // `to_ascii_lowercase` maps each byte to itself or another single ASCII
    // byte, so the index found still lands on the same offset in `comment`.
    let index = comment.to_ascii_lowercase().find("@disable")?;
    let after = &comment[index + "@disable".len()..];
    // Require a word boundary so `@disabled-rule` isn't mistaken for the
    // directive with an empty rule list.
    if after.starts_with(|c: char| !c.is_whitespace()) {
        return None;
    }

    let rest = after.trim();
    if rest.is_empty() {
        return Some(LineDisable::All);
    }

    let rules: HashSet<String> = rest
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    Some(LineDisable::Rules(rules))
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
}

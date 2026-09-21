//! Parses `@disable <rule-id>[, <rule-id>...]` and
//! `@disable-file <rule-id>[, <rule-id>...]` (also spelled `@file-disable`)
//! directives out of trailing `;`
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

use papyrus_parser::comment_annotations::{line_comment, parse_line_annotations};

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
    /// Scans `source` for `@disable`, `@disable-file`, and `@file-disable`
    /// directives in trailing line comments.
    pub fn scan(source: &str) -> Self {
        let mut lines = HashMap::new();
        let mut file = Vec::new();
        for (index, line) in source.lines().enumerate() {
            let line_number = index + 1;
            if let Some(directive) = parse_directive(line, "@disable") {
                lines.insert(line_number, directive);
            }
            for keyword in ["@disable-file", "@file-disable"] {
                if let Some(directive) = parse_directive(line, keyword) {
                    file.push((line_number, directive));
                }
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

/// Adds an `@disable` directive covering every rule id in `rules` to `line`
/// (1-indexed) of `source`, driving the code viewer's per-line "Ignore"
/// button. If `line` already carries a bare `@disable` (suppressing every
/// rule already), it's left untouched, since there's nothing left for it to
/// disable. If it already names some rules, any of `rules` not already
/// covered are merged into that same directive instead of appending a
/// second `@disable` -- a second occurrence would be invisible to
/// [`parse_directive`], which only ever looks for the first one in a line's
/// comment. An empty `rules` list, or a `line` outside `source`, leaves
/// `source` untouched.
pub(crate) fn add_disable_directive(source: &str, line: usize, rules: &[String]) -> String {
    add_named_disable_directive(source, line, rules, "@disable")
}

/// Adds an `@disable-file` directive covering every rule id in `rules` to
/// `line` (1-indexed) of `source`, driving the code viewer's per-line "File
/// disable" button. Merging and no-op rules match
/// [`add_disable_directive`], but against `@disable-file` rather than
/// `@disable`, so an existing line-level `@disable` on the same line is
/// left alone and a new `@disable-file` is appended beside it.
pub(crate) fn add_disable_file_directive(source: &str, line: usize, rules: &[String]) -> String {
    add_named_disable_directive(source, line, rules, "@disable-file")
}

fn add_named_disable_directive(
    source: &str,
    line: usize,
    rules: &[String],
    keyword: &str,
) -> String {
    if rules.is_empty() {
        return source.to_string();
    }
    let Some(index) = line.checked_sub(1) else {
        return source.to_string();
    };
    let lines: Vec<&str> = source.split('\n').collect();
    if lines.get(index).is_none() {
        return source.to_string();
    }
    let replaced = add_disable_directive_to_line(lines[index], rules, keyword);
    lines
        .iter()
        .enumerate()
        .map(|(i, original)| {
            if i == index {
                replaced.as_str()
            } else {
                original
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn add_disable_directive_to_line(line: &str, rules: &[String], keyword: &str) -> String {
    let (content, trailing_cr) = match line.strip_suffix('\r') {
        Some(stripped) => (stripped, "\r"),
        None => (line, ""),
    };

    match parse_directive(content, keyword) {
        Some(Directive::All { .. }) => line.to_string(),
        Some(Directive::Rules(existing)) => {
            let mut seen: HashSet<String> = existing.into_iter().map(|rule| rule.id).collect();
            let additions: Vec<&str> = rules
                .iter()
                .filter(|rule| seen.insert(rule.to_ascii_lowercase()))
                .map(String::as_str)
                .collect();
            if additions.is_empty() {
                return line.to_string();
            }
            let name = keyword.trim_start_matches('@');
            let end = parse_line_annotations(content)
                .into_iter()
                .find(|annotation| annotation.name.eq_ignore_ascii_case(name))
                .map_or(content.len(), |annotation| annotation.byte_end);
            format!(
                "{}, {}{}{}",
                &content[..end],
                additions.join(", "),
                &content[end..],
                trailing_cr
            )
        }
        None => {
            let separator = if line_comment_text(content).is_some() {
                " "
            } else {
                " ; "
            };
            format!(
                "{content}{separator}{keyword} {}{trailing_cr}",
                rules.join(", ")
            )
        }
    }
}

/// Finds a `keyword` (`@disable` or `@disable-file`) directive within
/// `line`'s trailing line comment, if it has one.
fn parse_directive(line: &str, keyword: &str) -> Option<Directive> {
    let name = keyword.trim_start_matches('@');
    let annotation = parse_line_annotations(line)
        .into_iter()
        .find(|annotation| annotation.name.eq_ignore_ascii_case(name))?;
    let rest = annotation.arguments;
    if rest.is_empty() {
        return Some(Directive::All {
            column: annotation.column,
        });
    }

    let rest_offset = annotation.arguments_byte_start;
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
    line_comment(line).map(|(_, comment)| comment)
}

#[cfg(test)]
#[path = "disable_comments_tests.rs"]
mod tests;

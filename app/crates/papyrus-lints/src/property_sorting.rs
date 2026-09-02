//! Flags script properties that aren't declared immediately after the
//! `ScriptName` line, sorted by type name and then alphabetically by
//! property name, and offers an automatic fix that reorders and relocates
//! them.
//!
//! Unlike every other lint in this crate, this one defaults to disabled
//! (see [`crate::config::Rules::property_sorting`]): reordering a script's
//! declared properties changes its structure more than a purely mechanical
//! style choice like trailing whitespace, so a project has to opt in.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a property declaration apart from other identifiers;
//! a script that doesn't parse cleanly is left unchecked rather than
//! guessed at. `Import` statements aren't tracked with a line number in
//! the AST and are never treated as blocking the property block from
//! following immediately after `ScriptName` — only a variable, function,
//! or state declaration appearing before a property counts as a
//! violation. The automatic fix moves only each property's own
//! declaration lines (its full `Property`/`EndProperty` block, for a
//! non-auto property); a documentation comment placed directly above a
//! property is left where it was rather than moved along with it.

use std::collections::BTreeSet;

use papyrus_parser::ast::{PropertyDecl, Script};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "property-sorting";

/// Checks `source` for properties that either aren't sorted by type and
/// then alphabetically by name, or aren't declared immediately after the
/// `ScriptName` line (before any variable, function, or state
/// declaration). Both are flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    if script.properties.is_empty() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

    if let Some(min_other_line) = other_member_lines(&script).into_iter().min() {
        for property in &script.properties {
            if property.line > min_other_line {
                diagnostics.push(Diagnostic {
                    line: property.line,
                    column: 1,
                    message: format!(
                        "[warning] Property '{}' must be declared immediately after the ScriptName declaration, before any variable, function, or state",
                        property.name
                    ),
                    rule: RULE,
                });
            }
        }
    }

    for pair in script.properties.windows(2) {
        let (prev, current) = (&pair[0], &pair[1]);
        if sort_key(current) < sort_key(prev) {
            diagnostics.push(Diagnostic {
                line: current.line,
                column: 1,
                message: format!(
                    "[warning] Property '{}' ({}) is out of order: properties must be sorted by type and then alphabetically, so it should come before '{}' ({})",
                    current.name,
                    display_type(current),
                    prev.name,
                    display_type(prev)
                ),
                rule: RULE,
            });
        }
    }

    diagnostics
}

/// Moves every property declaration to immediately follow the `ScriptName`
/// line, sorted by type and then alphabetically by name. A script that
/// doesn't parse cleanly, or that already satisfies [`check`], is
/// returned unchanged.
pub fn repair(source: &str) -> String {
    if check(source).is_empty() {
        return source.to_string();
    }
    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let scriptname_line = script.line;

    let lines: Vec<&str> = source.lines().collect();
    let chunks = line_chunks(source);
    if chunks.is_empty() {
        return source.to_string();
    }
    let newline = dominant_newline(&chunks);

    let with_spans: Vec<(&PropertyDecl, usize, usize)> = script
        .properties
        .iter()
        .map(|property| {
            let (start, end) = property_span(&lines, property);
            (property, start, end)
        })
        .collect();

    let mut removed = BTreeSet::new();
    for (_, start, end) in &with_spans {
        for line in *start..=*end {
            removed.insert(line);
        }
        if *end < lines.len() && lines[*end].trim().is_empty() {
            removed.insert(end + 1);
        }
    }

    let mut sorted = with_spans.clone();
    sorted.sort_by(|a, b| sort_key(a.0).cmp(&sort_key(b.0)));

    let total_lines = chunks.len();
    let mut result = String::with_capacity(source.len() + 64);

    for line_number in 1..=total_lines {
        if line_number == scriptname_line {
            result.push_str(chunks[line_number - 1]);
            for (index, (_, start, end)) in sorted.iter().enumerate() {
                for line in *start..=*end {
                    result.push_str(chunks[line - 1]);
                }
                if !chunks[*end - 1].ends_with('\n') {
                    result.push_str(newline);
                }
                if index + 1 < sorted.len() {
                    result.push_str(newline);
                }
            }
            let next_kept_line = ((line_number + 1)..=total_lines).find(|l| !removed.contains(l));
            if let Some(next_line) = next_kept_line {
                if !lines[next_line - 1].trim().is_empty() {
                    result.push_str(newline);
                }
            }
            continue;
        }
        if removed.contains(&line_number) {
            continue;
        }
        result.push_str(chunks[line_number - 1]);
    }

    result
}

/// The line of every top-level variable, function, or state declaration
/// (including functions declared inside a state), used to check that no
/// such declaration precedes a property. `Import` statements have no line
/// tracked on the AST and are deliberately not included here — see the
/// module docs.
fn other_member_lines(script: &Script) -> Vec<usize> {
    let mut lines: Vec<usize> = script.variables.iter().map(|v| v.line).collect();
    lines.extend(script.functions.iter().map(|f| f.line));
    for state in &script.states {
        lines.push(state.line);
        lines.extend(state.functions.iter().map(|f| f.line));
    }
    lines
}

fn type_key(property: &PropertyDecl) -> String {
    let mut key = property.type_name.name.to_lowercase();
    if property.type_name.is_array {
        key.push_str("[]");
    }
    key
}

fn sort_key(property: &PropertyDecl) -> (String, String) {
    (type_key(property), property.name.to_lowercase())
}

fn display_type(property: &PropertyDecl) -> String {
    if property.type_name.is_array {
        format!("{}[]", property.type_name.name)
    } else {
        property.type_name.name.clone()
    }
}

/// Whether `line` (with no line ending, as returned by [`str::lines`]) is
/// an `EndProperty` line, closing a full `Property`/`EndProperty` block,
/// optionally followed by whitespace or a trailing `;` comment.
fn is_end_property_line(line: &str) -> bool {
    const KEYWORD: &str = "endproperty";
    let trimmed = line.trim_start();
    if trimmed.len() < KEYWORD.len() || !trimmed[..KEYWORD.len()].eq_ignore_ascii_case(KEYWORD) {
        return false;
    }
    matches!(
        trimmed.as_bytes().get(KEYWORD.len()),
        None | Some(b' ') | Some(b'\t') | Some(b';')
    )
}

/// The 1-indexed (start, end) line span of `property`'s own declaration:
/// a single line for an `Auto`/`AutoReadOnly` property, or from its type
/// name through the matching `EndProperty` line for a full property.
/// `lines` is `source.lines()` collected, i.e. with no line endings.
fn property_span(lines: &[&str], property: &PropertyDecl) -> (usize, usize) {
    if property.is_auto || property.is_auto_read_only {
        return (property.line, property.line);
    }
    for (offset, line) in lines.iter().enumerate().skip(property.line) {
        if is_end_property_line(line) {
            return (property.line, offset + 1);
        }
    }
    (property.line, lines.len())
}

/// Splits `source` into chunks, one per line, each including its own line
/// ending (`\n` or `\r\n`) except a final line with none.
fn line_chunks(source: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        let (chunk, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        chunks.push(chunk);
        rest = remainder;
    }
    chunks
}

/// The line ending used by the first line of `chunks` that has one,
/// falling back to `\n` for a source with no line endings at all.
fn dominant_newline(chunks: &[&str]) -> &'static str {
    for chunk in chunks {
        if chunk.ends_with("\r\n") {
            return "\r\n";
        }
        if chunk.ends_with('\n') {
            return "\n";
        }
    }
    "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_already_sorted_and_positioned_properties() {
        let source =
            "ScriptName Example\n\nActor Property PlayerRef Auto\nInt Property Count = 1 Auto\n\nFunction DoThing()\nEndFunction\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn flags_property_out_of_type_order() {
        let source =
            "ScriptName Example\n\nInt Property Count = 1 Auto\nActor Property PlayerRef Auto\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("PlayerRef"));
        assert!(diagnostics[0].message.contains("Count"));
    }

    #[test]
    fn flags_property_out_of_name_order_within_the_same_type() {
        let source =
            "ScriptName Example\n\nInt Property Zulu = 1 Auto\nInt Property Alpha = 1 Auto\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Alpha"));
        assert!(diagnostics[0].message.contains("Zulu"));
    }

    #[test]
    fn flags_property_declared_after_a_function() {
        let source =
            "ScriptName Example\n\nFunction DoThing()\nEndFunction\n\nInt Property Count = 1 Auto\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);
        assert!(diagnostics[0].message.contains("Count"));
        assert!(diagnostics[0].message.contains("ScriptName"));
    }

    #[test]
    fn flags_only_the_property_declared_after_other_members() {
        let source =
            "ScriptName Example\n\nInt Property Early = 1 Auto\n\nFunction DoThing()\nEndFunction\n\nInt Property Late = 1 Auto\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Late"));
    }

    #[test]
    fn does_not_flag_a_single_property() {
        assert!(check("ScriptName Example\n\nInt Property Count = 1 Auto\n").is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nInt Property Count = \"unterminated\n").is_empty());
    }

    #[test]
    fn repair_sorts_by_type_then_name() {
        let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\n\nActor Property PlayerRef Auto\n\nInt Property Alpha = 1 Auto\n";
        let repaired = repair(source);

        assert!(repaired.starts_with("ScriptName Example\n"));
        let player_ref = repaired.find("PlayerRef").unwrap();
        let alpha = repaired.find("Alpha").unwrap();
        let zulu = repaired.find("Zulu").unwrap();
        assert!(player_ref < alpha && alpha < zulu);
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_relocates_a_property_declared_after_a_function() {
        let source =
            "ScriptName Example\n\nFunction DoThing()\nEndFunction\n\nInt Property Count = 1 Auto\n";
        let repaired = repair(source);

        assert!(repaired.starts_with("ScriptName Example\n"));
        assert!(
            repaired.find("Property Count").unwrap() < repaired.find("Function DoThing").unwrap()
        );
        assert!(repaired.contains("Function DoThing()\nEndFunction\n"));
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_moves_a_full_property_block_intact() {
        let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\n\nInt Property Alpha\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n";
        let repaired = repair(source);

        assert!(repaired.contains(
            "Int Property Alpha\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n"
        ));
        assert!(repaired.find("Alpha").unwrap() < repaired.find("Zulu").unwrap());
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_leaves_already_conforming_source_untouched() {
        let source =
            "ScriptName Example\n\nActor Property PlayerRef Auto\nInt Property Count = 1 Auto\n\nFunction DoThing()\nEndFunction\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_preserves_crlf_line_endings() {
        let source = "ScriptName Example\r\n\r\nInt Property Zulu = 1 Auto\r\n\r\nActor Property PlayerRef Auto\r\n";
        let repaired = repair(source);

        assert!(!repaired.contains("\r\n\r\n\n"));
        assert!(repaired.matches("\r\n").count() >= 3);
        assert!(!repaired.replace("\r\n", "").contains('\n'));
        assert!(repaired.find("PlayerRef").unwrap() < repaired.find("Zulu").unwrap());
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repair_does_not_crash_on_unparseable_source() {
        let source = "ScriptName Example\n\nInt Property Count = \"unterminated\n";
        assert_eq!(repair(source), source);
    }
}

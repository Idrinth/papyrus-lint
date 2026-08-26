//! Enforces a configured trailing-semicolon style.

use crate::Diagnostic;

/// The supported trailing-semicolon policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Require,
    Forbid,
}

/// Checks non-empty lines for the configured trailing-semicolon style.
pub fn check(source: &str, style: Style) -> Vec<Diagnostic> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let content = line.trim_end_matches([' ', '\t', '\r']);
            if content.is_empty() {
                return None;
            }

            let has_semicolon = content.ends_with(';');
            let message = match (style, has_semicolon) {
                (Style::Require, false) => "Line should end with a semicolon",
                (Style::Forbid, true) => "Line should not end with a semicolon",
                _ => return None,
            };

            Some(Diagnostic {
                line: index + 1,
                column: content.chars().count() + usize::from(!has_semicolon),
                message: message.to_string(),
            })
        })
        .collect()
}

/// Adds or removes terminal semicolons while retaining line endings. In
/// forbid mode only terminal semicolons are removed, so comment text is never
/// discarded.
pub fn repair(source: &str, style: Style) -> String {
    let mut result = String::with_capacity(source.len());

    for line_and_ending in source.split_inclusive('\n') {
        let (line, ending) = line_and_ending.strip_suffix("\r\n").map_or_else(
            || {
                line_and_ending
                    .strip_suffix('\n')
                    .map_or((line_and_ending, ""), |line| (line, "\n"))
            },
            |line| (line, "\r\n"),
        );
        repair_line(&mut result, line, style);
        result.push_str(ending);
    }

    result
}

fn repair_line(result: &mut String, line: &str, style: Style) {
    let content = line.trim_end_matches([' ', '\t']);
    let whitespace = &line[content.len()..];

    match style {
        Style::Require if !content.is_empty() && !content.ends_with(';') => {
            result.push_str(content);
            result.push(';');
            result.push_str(whitespace);
        }
        Style::Forbid if content.ends_with(';') => {
            result.push_str(&content[..content.len() - 1]);
            result.push_str(whitespace);
        }
        _ => result.push_str(line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_flags_and_repairs_missing_semicolons() {
        let source = "ScriptName Example\n\nInt value = 1;\r\n";
        assert_eq!(check(source, Style::Require).len(), 1);
        assert_eq!(
            repair(source, Style::Require),
            "ScriptName Example;\n\nInt value = 1;\r\n"
        );
    }

    #[test]
    fn forbid_only_removes_terminal_semicolons() {
        let source = "Int value = 1;  \nDebug.Trace(\"x\") ; explanation\n";
        assert_eq!(check(source, Style::Forbid).len(), 1);
        assert_eq!(
            repair(source, Style::Forbid),
            "Int value = 1  \nDebug.Trace(\"x\") ; explanation\n"
        );
    }

    #[test]
    fn repair_is_idempotent() {
        for style in [Style::Require, Style::Forbid] {
            let repaired = repair("a  \r\n;b\n", style);
            assert_eq!(repair(&repaired, style), repaired);
            assert!(check(&repaired, style).is_empty());
        }
    }
}

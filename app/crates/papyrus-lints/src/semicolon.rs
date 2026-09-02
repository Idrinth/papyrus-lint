//! Enforces a configured trailing-semicolon style.

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "semicolon";

/// The supported trailing-semicolon policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Require,
    Forbid,
}

/// Checks non-empty lines for the configured trailing-semicolon style.
/// Lines inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// are never flagged.
pub fn check(source: &str, style: Style) -> Vec<Diagnostic> {
    let protected = fragment_code::protected_lines(source);

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if protected[index + 1] {
                return None;
            }

            let content = line.trim_end_matches([' ', '\t', '\r']);
            if content.is_empty() {
                return None;
            }

            let has_semicolon = content.ends_with(';');
            let message = match (style, has_semicolon) {
                (Style::Require, false) => "[warning] Line should end with a semicolon",
                (Style::Forbid, true) => "[warning] Line should not end with a semicolon",
                _ => return None,
            };

            Some(Diagnostic {
                line: index + 1,
                column: content.chars().count() + usize::from(!has_semicolon),
                message: message.to_string(),
                rule: RULE,
            })
        })
        .collect()
}

/// Adds or removes terminal semicolons while retaining line endings. In
/// forbid mode only terminal semicolons are removed, so comment text is never
/// discarded. Lines protected by a CreationKit fragment-code wrapper (see
/// [`fragment_code`]) are left exactly as-is.
pub fn repair(source: &str, style: Style) -> String {
    let protected = fragment_code::protected_lines(source);
    let mut result = String::with_capacity(source.len());
    for (line_number, line_and_ending) in (1usize..).zip(source.split_inclusive('\n')) {
        if protected[line_number] {
            result.push_str(line_and_ending);
        } else {
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
    fn fragment_code_wrapper_is_never_touched() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        let diagnostics = check(source, Style::Require);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 5);

        let repaired = repair(source, Style::Require);
        assert_eq!(
            repaired,
            "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef);
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
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

    #[test]
    fn require_ignores_whitespace_only_lines_and_preserves_it() {
        let source = " \t\r\nvalue\t\n";
        let diagnostics = check(source, Style::Require);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 6));
        assert_eq!(repair(source, Style::Require), " \t\r\nvalue;\t\n");
    }

    #[test]
    fn diagnostic_columns_count_characters_instead_of_utf8_bytes() {
        let diagnostics = check("String greeting = \"hé\"\n", Style::Require);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 23);
    }

    #[test]
    fn repair_handles_a_final_line_without_a_newline() {
        assert_eq!(repair("value", Style::Require), "value;");
        assert_eq!(repair("value;", Style::Forbid), "value");
    }

    #[test]
    fn check_reports_the_terminal_semicolon_location_and_rule() {
        let diagnostics = check("Int value = 1;  \n", Style::Forbid);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 14));
        assert_eq!(diagnostics[0].rule, RULE);
        assert_eq!(
            diagnostics[0].message,
            "[warning] Line should not end with a semicolon"
        );
    }

    #[test]
    fn require_preserves_trailing_whitespace_on_a_final_line() {
        let source = "value \t";

        assert_eq!(repair(source, Style::Require), "value; \t");
        assert!(check(&repair(source, Style::Require), Style::Require).is_empty());
    }

    #[test]
    fn matching_styles_leave_source_unchanged() {
        assert_eq!(repair("value;\n", Style::Require), "value;\n");
        assert_eq!(repair("value\n", Style::Forbid), "value\n");
        assert!(check("value;\n", Style::Require).is_empty());
        assert!(check("value\n", Style::Forbid).is_empty());
    }
}

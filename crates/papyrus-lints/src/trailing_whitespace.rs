//! Flags lines that end with trailing spaces or tabs.

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "trailing-whitespace";

const TRAILING_WHITESPACE: [char; 2] = [' ', '\t'];

/// Checks `source` for lines ending in trailing spaces or tabs. Lines
/// inside a CreationKit fragment-code wrapper (see [`fragment_code`]),
/// outside of its `;BEGIN CODE`/`;END CODE` markers, are never flagged.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let protected = fragment_code::protected_lines(source);

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if protected[index + 1] {
                return None;
            }

            let trimmed = line.trim_end_matches(TRAILING_WHITESPACE);
            if trimmed.len() == line.len() {
                return None;
            }

            Some(Diagnostic {
                line: index + 1,
                column: trimmed.chars().count() + 1,
                message: "[warning] Line contains trailing whitespace".to_string(),
                rule: RULE,
            })
        })
        .collect()
}

/// Strips trailing spaces/tabs from every line of `source`, preserving each
/// line's original ending (`\n`, `\r\n`, or none for a final line without a
/// trailing newline) and leaving lines that have no trailing whitespace
/// untouched. Lines protected by a CreationKit fragment-code wrapper (see
/// [`fragment_code`]) are left exactly as-is.
pub fn repair(source: &str) -> String {
    let protected = fragment_code::protected_lines(source);
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;

    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };

        if protected[line_number] {
            result.push_str(line_and_ending);
        } else {
            let (content, ending) = if let Some(stripped) = line_and_ending.strip_suffix("\r\n") {
                (stripped, "\r\n")
            } else if let Some(stripped) = line_and_ending.strip_suffix('\n') {
                (stripped, "\n")
            } else {
                (line_and_ending, "")
            };

            result.push_str(content.trim_end_matches(TRAILING_WHITESPACE));
            result.push_str(ending);
        }

        rest = remainder;
        line_number += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_trailing_spaces() {
        let diagnostics = check("ScriptName Example  \n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 19);
    }

    #[test]
    fn flags_trailing_tabs() {
        let diagnostics = check("Int x = 1\t\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 10);
    }

    #[test]
    fn ignores_clean_lines() {
        let diagnostics = check("ScriptName Example\n\nInt x = 1\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_trailing_carriage_return() {
        let diagnostics = check("ScriptName Example\r\nInt x = 1\r\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_multiple_lines_independently() {
        let source = "Line one \nLine two\nLine three\t\n";
        let diagnostics = check(source);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[1].line, 3);
    }

    #[test]
    fn flags_whitespace_only_line() {
        let diagnostics = check("ScriptName Example\n   \nInt x = 1\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].column, 1);
    }

    #[test]
    fn flags_last_line_without_trailing_newline() {
        let diagnostics = check("ScriptName Example   ");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn repairs_trailing_spaces() {
        assert_eq!(repair("ScriptName Example  \n"), "ScriptName Example\n");
    }

    #[test]
    fn repairs_trailing_tabs() {
        assert_eq!(repair("Int x = 1\t\n"), "Int x = 1\n");
    }

    #[test]
    fn leaves_clean_lines_untouched() {
        let source = "ScriptName Example\n\nInt x = 1\n";
        assert_eq!(repair(source), source);
    }

    #[test]
    fn preserves_crlf_line_endings() {
        assert_eq!(repair("Int x = 1  \r\n"), "Int x = 1\r\n");
    }

    #[test]
    fn preserves_leading_whitespace_and_line_without_trailing_newline() {
        assert_eq!(repair("\tInt x = 1   "), "\tInt x = 1");
    }

    #[test]
    fn clears_whitespace_only_lines() {
        assert_eq!(
            repair("ScriptName Example\n   \nInt x = 1\n"),
            "ScriptName Example\n\nInt x = 1\n"
        );
    }

    #[test]
    fn repairs_multiple_lines_independently() {
        let source = "Line one \nLine two\nLine three\t\n";
        assert_eq!(repair(source), "Line one\nLine two\nLine three\n");
    }

    #[test]
    fn fragment_code_wrapper_trailing_whitespace_is_left_alone() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment  \nScriptname Example Extends TopicInfo Hidden\nFunction Fragment_0(ObjectReference akSpeakerRef)\n;BEGIN CODE\nakSpeaker.RemoveItem(x, 1, false, PlayerRef)  \n;END CODE\nEndFunction\t\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";
        assert!(check(source).iter().all(|d| d.line == 5));
        let repaired = repair(source);
        assert!(repaired.starts_with(
            ";BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment  \n"
        ));
        assert!(repaired.contains("EndFunction\t\n"));
        assert!(repaired.contains("akSpeaker.RemoveItem(x, 1, false, PlayerRef)\n"));
    }

    #[test]
    fn repaired_source_has_no_remaining_diagnostics() {
        let source = "Line one \r\nLine two\t\n\tLine three   ";
        let repaired = repair(source);
        assert!(check(&repaired).is_empty());
    }
}

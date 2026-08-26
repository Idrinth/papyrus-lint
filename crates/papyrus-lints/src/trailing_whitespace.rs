//! Flags lines that end with trailing spaces or tabs.

use crate::Diagnostic;

const TRAILING_WHITESPACE: [char; 2] = [' ', '\t'];

/// Checks `source` for lines ending in trailing spaces or tabs.
pub fn check(source: &str) -> Vec<Diagnostic> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_end_matches(TRAILING_WHITESPACE);
            if trimmed.len() == line.len() {
                return None;
            }

            Some(Diagnostic {
                line: index + 1,
                column: trimmed.chars().count() + 1,
                message: "Line contains trailing whitespace".to_string(),
            })
        })
        .collect()
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
}

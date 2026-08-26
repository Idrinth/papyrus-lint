//! Re-indents Papyrus block statements with a configurable indentation unit.

use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{Keyword, TokenKind};

/// The indentation unit to use for each level of nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum Indentation {
    Tabs,
    Spaces(usize),
}

impl Indentation {
    fn unit(self) -> String {
        match self {
            Self::Tabs => "\t".to_string(),
            Self::Spaces(count) => " ".repeat(count),
        }
    }
}

/// Replaces leading whitespace with the configured indentation while preserving
/// line endings, blank lines, and all non-leading content.
pub fn repair(source: &str, indentation: Indentation) -> String {
    let unit = indentation.unit();
    let mut keywords_by_line = vec![Vec::new(); source.lines().count() + 1];

    // Do not risk changing a file whose structure cannot be identified.
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return source.to_string();
    };
    for token in tokens {
        if let TokenKind::Keyword(keyword) = token.kind {
            if let Some(keywords) = keywords_by_line.get_mut(token.line) {
                keywords.push(keyword);
            }
        }
    }

    let mut result = String::with_capacity(source.len());
    let mut depth = 0usize;
    let mut rest = source;
    let mut line_number = 1usize;

    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        let (line, ending) = if let Some(line) = line_and_ending.strip_suffix("\r\n") {
            (line, "\r\n")
        } else if let Some(line) = line_and_ending.strip_suffix('\n') {
            (line, "\n")
        } else {
            (line_and_ending, "")
        };
        let keywords = &keywords_by_line[line_number];

        if closes_block(keywords) {
            depth = depth.saturating_sub(1);
        }

        let content = line.trim_start_matches([' ', '\t']);
        if !content.is_empty() {
            result.push_str(&unit.repeat(depth));
            result.push_str(content);
        }
        result.push_str(ending);

        if opens_block(keywords) {
            depth += 1;
        }
        rest = remainder;
        line_number += 1;
    }

    result
}

fn closes_block(keywords: &[Keyword]) -> bool {
    keywords.iter().any(|keyword| {
        matches!(
            keyword,
            Keyword::EndFunction
                | Keyword::EndEvent
                | Keyword::EndProperty
                | Keyword::EndIf
                | Keyword::EndWhile
                | Keyword::EndState
                | Keyword::Else
                | Keyword::ElseIf
        )
    })
}

fn opens_block(keywords: &[Keyword]) -> bool {
    if keywords
        .iter()
        .any(|keyword| matches!(keyword, Keyword::Else | Keyword::ElseIf))
    {
        return true;
    }

    keywords.iter().any(|keyword| match keyword {
        Keyword::If | Keyword::While | Keyword::State | Keyword::Event => true,
        Keyword::Function => !keywords.contains(&Keyword::Native),
        Keyword::Property => {
            !keywords.contains(&Keyword::Auto) && !keywords.contains(&Keyword::AutoReadOnly)
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "ScriptName Example\nFunction Run()\nIf ready\nDoThing()\nElseIf waiting\nWait()\nElse\nStop()\nEndIf\nEndFunction\n";

    #[test]
    fn repairs_nested_blocks_with_spaces() {
        assert_eq!(
            repair(SOURCE, Indentation::Spaces(2)),
            "ScriptName Example\nFunction Run()\n  If ready\n    DoThing()\n  ElseIf waiting\n    Wait()\n  Else\n    Stop()\n  EndIf\nEndFunction\n"
        );
    }

    #[test]
    fn repairs_nested_blocks_with_tabs() {
        let repaired = repair(SOURCE, Indentation::Tabs);
        assert!(repaired.contains("\tIf ready\n\t\tDoThing()\n"));
    }

    #[test]
    fn preserves_crlf_blank_lines_and_final_line_ending() {
        let source = "Function Run()\r\n  \r\n    Return\r\nEndFunction";
        assert_eq!(
            repair(source, Indentation::Spaces(4)),
            "Function Run()\r\n\r\n    Return\r\nEndFunction"
        );
    }

    #[test]
    fn native_functions_and_auto_properties_do_not_open_blocks() {
        let source = "Int Function GetValue() Native\nInt Property Value Auto\nInt x\n";
        assert_eq!(repair(source, Indentation::Spaces(2)), source);
    }

    #[test]
    fn repair_is_idempotent() {
        let repaired = repair(SOURCE, Indentation::Spaces(3));
        assert_eq!(repair(&repaired, Indentation::Spaces(3)), repaired);
    }

    #[test]
    fn malformed_source_is_left_unchanged() {
        let source = "Function Run()\n  @invalid\nEndFunction\n";
        assert_eq!(repair(source, Indentation::Tabs), source);
    }

    #[test]
    fn deserializes_frontend_configuration() {
        assert_eq!(
            serde_json::from_str::<Indentation>(r#""Tabs""#).unwrap(),
            Indentation::Tabs
        );
        assert_eq!(
            serde_json::from_str::<Indentation>(r#"{"Spaces":4}"#).unwrap(),
            Indentation::Spaces(4)
        );
    }
}

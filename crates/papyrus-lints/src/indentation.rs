//! Flags and repairs Papyrus block statements whose indentation doesn't
//! match a configurable indentation unit.

use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{Keyword, TokenKind};

use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "indentation";

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

    fn describe(self, depth: usize) -> String {
        match self {
            Self::Tabs => {
                let noun = if depth == 1 { "tab" } else { "tabs" };
                format!("{depth} {noun}")
            }
            Self::Spaces(count) => {
                let width = depth * count;
                let noun = if width == 1 { "space" } else { "spaces" };
                format!("{width} {noun}")
            }
        }
    }
}

/// Determines each line's expected nesting depth from `source`'s block
/// keywords (`If`/`EndIf`, `Function`/`EndFunction`, ...), returning `None`
/// if `source`'s structure can't be identified (e.g. it doesn't lex
/// cleanly).
fn line_depths(source: &str) -> Option<Vec<usize>> {
    let mut keywords_by_line = vec![Vec::new(); source.lines().count() + 1];

    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return None;
    };
    for token in tokens {
        if let TokenKind::Keyword(keyword) = token.kind {
            if let Some(keywords) = keywords_by_line.get_mut(token.line) {
                keywords.push(keyword);
            }
        }
    }

    let mut depths = vec![0usize; keywords_by_line.len()];
    let mut depth = 0usize;
    for (line_number, keywords) in keywords_by_line.iter().enumerate().skip(1) {
        if closes_block(keywords) {
            depth = depth.saturating_sub(1);
        }
        depths[line_number] = depth;
        if opens_block(keywords) {
            depth += 1;
        }
    }

    Some(depths)
}

/// Checks `source` for lines whose leading whitespace doesn't match the
/// indentation expected at their nesting depth. Blank/whitespace-only lines
/// are never flagged. Returns no diagnostics if `source`'s structure can't
/// be identified, since there's nothing to compare against. Lines inside a
/// CreationKit fragment-code wrapper (see [`fragment_code`]), outside of
/// its `;BEGIN CODE`/`;END CODE` markers, are never flagged.
pub fn check(source: &str, indentation: Indentation) -> Vec<Diagnostic> {
    let Some(depths) = line_depths(source) else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);
    let unit = indentation.unit();

    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if protected[index + 1] {
                return None;
            }

            let content = line.trim_start_matches([' ', '\t']);
            if content.is_empty() {
                return None;
            }

            let depth = depths[index + 1];
            let leading = &line[..line.len() - content.len()];
            if leading == unit.repeat(depth) {
                return None;
            }

            Some(Diagnostic {
                line: index + 1,
                column: 1,
                message: format!(
                    "Line should be indented with {}",
                    indentation.describe(depth)
                ),
                rule: RULE,
            })
        })
        .collect()
}

/// Replaces leading whitespace with the configured indentation while preserving
/// line endings, blank lines, and all non-leading content. Lines protected
/// by a CreationKit fragment-code wrapper (see [`fragment_code`]) are left
/// exactly as-is.
pub fn repair(source: &str, indentation: Indentation) -> String {
    let unit = indentation.unit();

    // Do not risk changing a file whose structure cannot be identified.
    let Some(depths) = line_depths(source) else {
        return source.to_string();
    };
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
            let (line, ending) = if let Some(line) = line_and_ending.strip_suffix("\r\n") {
                (line, "\r\n")
            } else if let Some(line) = line_and_ending.strip_suffix('\n') {
                (line, "\n")
            } else {
                (line_and_ending, "")
            };

            let content = line.trim_start_matches([' ', '\t']);
            if !content.is_empty() {
                result.push_str(&unit.repeat(depths[line_number]));
                result.push_str(content);
            }
            result.push_str(ending);
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
    fn flags_lines_indented_with_the_wrong_unit() {
        let source = "Function Run()\n  If ready\nDoThing()\nEndIf\nEndFunction\n";
        let diagnostics = check(source, Indentation::Tabs);

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].column, 1);
        assert_eq!(diagnostics[0].message, "Line should be indented with 1 tab");
        assert_eq!(diagnostics[1].line, 3);
        assert_eq!(
            diagnostics[1].message,
            "Line should be indented with 2 tabs"
        );
        assert_eq!(diagnostics[2].line, 4);
        assert_eq!(diagnostics[2].message, "Line should be indented with 1 tab");
    }

    #[test]
    fn flags_lines_indented_with_the_wrong_width() {
        let source = "Function Run()\n If ready\n  DoThing()\n EndIf\nEndFunction\n";
        let diagnostics = check(source, Indentation::Spaces(2));

        assert_eq!(diagnostics.len(), 3);
        assert_eq!(
            diagnostics[0].message,
            "Line should be indented with 2 spaces"
        );
        assert_eq!(
            diagnostics[1].message,
            "Line should be indented with 4 spaces"
        );
        assert_eq!(
            diagnostics[2].message,
            "Line should be indented with 2 spaces"
        );
    }

    #[test]
    fn ignores_correctly_indented_source() {
        let repaired = repair(SOURCE, Indentation::Spaces(2));
        assert!(check(&repaired, Indentation::Spaces(2)).is_empty());
    }

    #[test]
    fn ignores_blank_lines() {
        let source = "Function Run()\n\n   \nEndFunction\n";
        assert!(check(source, Indentation::Tabs).is_empty());
    }

    #[test]
    fn ignores_malformed_source() {
        let source = "Function Run()\n  @invalid\nEndFunction\n";
        assert!(check(source, Indentation::Tabs).is_empty());
    }

    #[test]
    fn checking_repaired_output_finds_nothing() {
        for indentation in [Indentation::Tabs, Indentation::Spaces(3)] {
            let repaired = repair(SOURCE, indentation);
            assert!(check(&repaired, indentation).is_empty());
        }
    }

    #[test]
    fn fragment_code_wrapper_is_never_reindented() {
        // The function signature, the local variable declaration, `EndFunction`,
        // and every wrapper/marker comment must stay exactly as CreationKit wrote
        // them; only the actual code between `;BEGIN CODE`/`;END CODE` may be
        // reindented to match its nesting depth.
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
        let diagnostics = check(source, Indentation::Tabs);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 6);

        assert_eq!(
            repair(source, Indentation::Tabs),
            "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
\takSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
        );
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

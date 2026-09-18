//! Flags and repairs Papyrus block statements whose indentation doesn't
//! match a configurable indentation unit.

use papyrus_parser::token::{Keyword, Token, TokenKind};

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
    let tokens = papyrus_parser::tokenize(source).ok()?;
    Some(line_depths_from_tokens(source, &tokens))
}

/// Same as [`line_depths`], but from an already-lexed token stream instead
/// of lexing `source` itself; used by [`check`], which receives `tokens`
/// already computed by its caller.
fn line_depths_from_tokens(source: &str, tokens: &[Token]) -> Vec<usize> {
    let mut keywords_by_line = vec![Vec::new(); source.lines().count() + 1];

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

    depths
}

/// Checks `source` for lines whose leading whitespace doesn't match the
/// indentation expected at their nesting depth. Blank/whitespace-only lines
/// are never flagged. Returns no diagnostics if `source`'s structure can't
/// be identified, since there's nothing to compare against. Lines inside a
/// CreationKit fragment-code wrapper (see [`fragment_code`]), outside of
/// its `;BEGIN CODE`/`;END CODE` markers, are never flagged; lines inside
/// those markers are expected relative to the marker's own depth (see
/// [`fragment_code::code_section_starts`]), not the file-wide depth of the
/// (never-reindented) wrapper function around them.
pub fn check(source: &str, tokens: Option<&[Token]>, indentation: Indentation) -> Vec<Diagnostic> {
    let Some(tokens) = tokens else {
        return Vec::new();
    };
    let depths = line_depths_from_tokens(source, tokens);
    let protected = fragment_code::protected_lines(source);
    let code_section_starts = fragment_code::code_section_starts(source);
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

            let depth = expected_depth(&depths, code_section_starts[index + 1], index + 1);
            let leading = &line[..line.len() - content.len()];
            if leading == unit.repeat(depth) {
                return None;
            }

            Some(Diagnostic {
                line: index + 1,
                column: 1,
                message: format!(
                    "[warning] Line should be indented with {}",
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
/// exactly as-is; lines between a `;BEGIN CODE`/`;END CODE` pair are
/// indented relative to that marker's own depth rather than the file-wide
/// depth of the wrapper function around them (see [`check`]).
pub fn repair(source: &str, indentation: Indentation) -> String {
    let unit = indentation.unit();

    // Do not risk changing a file whose structure cannot be identified.
    let Some(depths) = line_depths(source) else {
        return source.to_string();
    };
    let protected = fragment_code::protected_lines(source);
    let code_section_starts = fragment_code::code_section_starts(source);

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
                let depth = expected_depth(&depths, code_section_starts[line_number], line_number);
                result.push_str(&unit.repeat(depth));
                result.push_str(content);
            }
            result.push_str(ending);
        }

        rest = remainder;
        line_number += 1;
    }

    result
}

/// Returns `line_number`'s expected indentation depth: the file-wide depth
/// from `depths`, unless `code_section_start` names the `;BEGIN CODE`
/// marker line that opened the fragment-code section `line_number` falls
/// inside, in which case the marker's own depth is subtracted so the
/// section's code is measured relative to the marker instead of the
/// (never-reindented) wrapper function around it.
fn expected_depth(
    depths: &[usize],
    code_section_start: Option<usize>,
    line_number: usize,
) -> usize {
    match code_section_start {
        Some(begin_line) => depths[line_number].saturating_sub(depths[begin_line]),
        None => depths[line_number],
    }
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
#[path = "indentation_tests.rs"]
mod tests;

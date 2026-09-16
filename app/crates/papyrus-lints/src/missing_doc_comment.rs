//! Flags, as a `[warning]`, a script header (`ScriptName`), `Property`
//! declaration, or `Function`/`Event` declaration with no documentation
//! comment on the line immediately following it, per CreationKit's own
//! `{ ... }` documentation comment syntax — rendered as a tooltip when
//! hovering the script in the script picker, or a property in the property
//! editor.
//!
//! Uses the parsed AST for the declarations' own line numbers, but checks
//! the raw source text for the comment itself, since the lexer discards a
//! `{ ... }` comment's contents entirely rather than keeping it as a token
//! (see `papyrus-parser`'s lexer's `skip_brace_comment`) — there's no AST
//! node left for this lint to inspect instead. Disabled by default, since
//! most existing scripts carry none of these comments at all and enabling
//! this would flag literally every declaration in such a project at once;
//! a project opts in via `rules.missing_doc_comment`.

use papyrus_parser::ast::{FunctionDecl, Script};
use papyrus_parser::token::{Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "missing-doc-comment";

/// Checks `source` for a script header, `Property`, or `Function`/`Event`
/// declaration with no `{ ... }` documentation comment on the line right
/// after it. A script that doesn't parse cleanly is left unchecked rather
/// than guessed at.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return Vec::new();
    };

    let lines: Vec<&str> = source.split('\n').collect();
    let mut diagnostics = Vec::new();

    check_declaration(
        script.line,
        format!("The `ScriptName {}` declaration", script.name),
        &tokens,
        &lines,
        &mut diagnostics,
    );

    for property in &script.properties {
        check_declaration(
            property.line,
            format!("Property `{}`", property.name),
            &tokens,
            &lines,
            &mut diagnostics,
        );
    }

    for function in all_functions(&script) {
        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };
        check_declaration(
            function.line,
            format!("{kind} `{}`", function.name),
            &tokens,
            &lines,
            &mut diagnostics,
        );
    }

    diagnostics
}

/// The `{ ... }` documentation comment immediately following the
/// declaration that starts on `line` (1-indexed), if any. Placement
/// matches [`check`]: after the header's last physical line
/// (backslash-continued headers included). Returns the comment's inner
/// text with the surrounding `{`/`}` stripped and leading/trailing
/// whitespace trimmed; multi-line comments keep their inner newlines.
/// Empty `{ }` comments (which still satisfy [`check`]) yield `None`,
/// since there's nothing to show a tooltip.
pub fn documentation_comment(source: &str, tokens: &[Token], line: usize) -> Option<String> {
    brace_comment_starting_on_line(source, last_physical_line(line, tokens) + 1)
}

/// Every function declared directly on the script, plus every function
/// declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Flags `subject` (rooted at `line`, 1-indexed) if the raw source line
/// immediately following its actual last physical line (see
/// [`last_physical_line`]) doesn't start (after leading whitespace) with a
/// documentation comment's opening `{`. The diagnostic itself is still
/// reported at `line`, the declaration's own start, regardless of where its
/// header actually ends.
fn check_declaration(
    line: usize,
    subject: String,
    tokens: &[Token],
    lines: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let last_line = last_physical_line(line, tokens);
    let has_doc_comment = lines
        .get(last_line)
        .is_some_and(|next_line| next_line.trim_start().starts_with('{'));
    if has_doc_comment {
        return;
    }
    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[warning] {subject} has no documentation comment (`{{...}}`) on the line \
             immediately following it"
        ),
        rule: RULE,
    });
}

/// The last physical source line (1-indexed) of the logical line starting
/// at `line`: the line the first `Newline` token at or after `line` itself
/// falls on. A header written entirely on one physical line just returns
/// `line` back, since that line's own terminating newline is the first
/// (and only) `Newline` token at or after it. A header continued onto
/// further physical lines via a trailing `\` — which `papyrus-parser`'s
/// lexer swallows together with the newline right after it, emitting no
/// `Newline` token for that line at all — instead resolves to whichever
/// later physical line the header's real terminating newline falls on, so
/// its documentation comment is looked for after that line rather than
/// after the header's first, continued line.
fn last_physical_line(line: usize, tokens: &[Token]) -> usize {
    tokens
        .iter()
        .find(|token| token.kind == TokenKind::Newline && token.line >= line)
        .map_or(line, |token| token.line)
}

/// Inner text of a `{ ... }` comment whose opening `{` is the first
/// non-whitespace character on 1-indexed `line`. Mirrors the lexer's
/// `skip_brace_comment`: the comment runs to the first `}`, and may span
/// further physical lines. Returns `None` when that line doesn't open a
/// comment, the comment is unterminated, or its inner text is empty.
fn brace_comment_starting_on_line(source: &str, line: usize) -> Option<String> {
    if line == 0 {
        return None;
    }
    let mut current_line = 1usize;
    let mut line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if current_line == line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = idx + 1;
        }
    }
    if current_line != line {
        return None;
    }
    let rest = &source[line_start..];
    let trimmed = rest.trim_start_matches([' ', '\t', '\r']);
    let skipped = rest.len() - trimmed.len();
    if rest[..skipped].contains('\n') {
        return None;
    }
    let mut chars = trimmed.chars();
    if chars.next() != Some('{') {
        return None;
    }
    let inner_with_tail = chars.as_str();
    let end = inner_with_tail.find('}')?;
    let inner = inner_with_tail[..end]
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let text = inner.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_of(source: &str) -> Vec<Token> {
        papyrus_parser::tokenize(source).expect("test source should tokenize")
    }

    #[test]
    fn flags_script_header_with_no_doc_comment() {
        let diagnostics = check("ScriptName Example\n\nFunction Test()\nEndFunction\n");

        let script_finding = diagnostics
            .iter()
            .find(|d| d.message.contains("ScriptName Example"))
            .expect("script header should be flagged");
        assert_eq!(script_finding.line, 1);
        assert_eq!(script_finding.column, 1);
        assert_eq!(script_finding.rule, RULE);
        assert_eq!(script_finding.level(), "warning");
    }

    #[test]
    fn does_not_flag_script_header_with_a_doc_comment() {
        let diagnostics = check(
            "ScriptName Example\n{Documentation for my cool script here!}\n\nFunction Test()\nEndFunction\n",
        );

        assert!(diagnostics
            .iter()
            .all(|d| !d.message.contains("ScriptName Example")));
    }

    #[test]
    fn flags_property_with_no_doc_comment() {
        let diagnostics = check("ScriptName Example\n\nInt Property MyProperty Auto\n");

        let property_finding = diagnostics
            .iter()
            .find(|d| d.message.contains("Property `MyProperty`"))
            .expect("property should be flagged");
        assert_eq!(property_finding.line, 3);
        assert_eq!(property_finding.column, 1);
    }

    #[test]
    fn does_not_flag_property_with_a_doc_comment() {
        let diagnostics = check(
            "ScriptName Example\n{doc}\n\nInt Property MyProperty Auto\n{This property is fun, if you set it to 1, watch cool stuff happen!}\n",
        );

        assert!(diagnostics
            .iter()
            .all(|d| !d.message.contains("Property `MyProperty`")));
    }

    #[test]
    fn flags_function_with_no_doc_comment() {
        let diagnostics = check("ScriptName Example\n{doc}\n\nFunction DoThing()\nEndFunction\n");

        let function_finding = diagnostics
            .iter()
            .find(|d| d.message.contains("Function `DoThing`"))
            .expect("function should be flagged");
        assert_eq!(function_finding.line, 4);
    }

    #[test]
    fn does_not_flag_function_with_a_doc_comment() {
        let diagnostics = check(
            "ScriptName Example\n{doc}\n\nFunction DoThing()\n{Explains what DoThing does}\nEndFunction\n",
        );

        assert!(diagnostics
            .iter()
            .all(|d| !d.message.contains("Function `DoThing`")));
    }

    #[test]
    fn flags_event_with_no_doc_comment() {
        let diagnostics = check("ScriptName Example\n{doc}\n\nEvent OnInit()\nEndEvent\n");

        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("Event `OnInit`")));
    }

    #[test]
    fn checks_functions_declared_inside_states_too() {
        let diagnostics = check(
            "ScriptName Example\n{doc}\n\nState Active\n    Function DoThing()\n    EndFunction\nEndState\n",
        );

        let function_finding = diagnostics
            .iter()
            .find(|d| d.message.contains("Function `DoThing`"))
            .expect("state function should be flagged");
        assert_eq!(function_finding.line, 5);
    }

    #[test]
    fn a_multi_line_doc_comment_still_counts() {
        let diagnostics = check(
            "ScriptName Example\n{Documentation for my cool script here!\nI can even use more than one line...}\n\nFunction Test()\nEndFunction\n",
        );

        assert!(diagnostics
            .iter()
            .all(|d| !d.message.contains("ScriptName Example")));
    }

    #[test]
    fn a_backslash_continued_function_header_checks_its_real_next_line() {
        let diagnostics = check(
            "ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\n{Explains what DoThing does}\nEndFunction\n",
        );

        assert!(diagnostics
            .iter()
            .all(|d| !d.message.contains("Function `DoThing`")));
    }

    #[test]
    fn a_backslash_continued_function_header_without_a_doc_comment_is_still_flagged() {
        let diagnostics = check(
            "ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\nEndFunction\n",
        );

        let function_finding = diagnostics
            .iter()
            .find(|d| d.message.contains("Function `DoThing`"))
            .expect("function should be flagged");
        assert_eq!(function_finding.line, 4);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn a_declaration_on_the_last_line_with_no_following_line_is_flagged() {
        let diagnostics = check("ScriptName Example\n\nInt Property MyProperty Auto");

        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("Property `MyProperty`")));
    }

    #[test]
    fn documentation_comment_returns_the_inner_text() {
        let source =
            "ScriptName Example\n{Documentation for my cool script here!}\n\nFunction Test()\nEndFunction\n";
        let tokens = tokens_of(source);

        assert_eq!(
            documentation_comment(source, &tokens, 1).as_deref(),
            Some("Documentation for my cool script here!")
        );
    }

    #[test]
    fn documentation_comment_keeps_inner_newlines_of_a_multi_line_comment() {
        let source =
            "ScriptName Example\n{Documentation for my cool script here!\nI can even use more than one line...}\n";
        let tokens = tokens_of(source);

        assert_eq!(
            documentation_comment(source, &tokens, 1).as_deref(),
            Some("Documentation for my cool script here!\nI can even use more than one line...")
        );
    }

    #[test]
    fn documentation_comment_follows_a_backslash_continued_header() {
        let source =
            "ScriptName Example\n{doc}\n\nFunction DoThing(Int a, \\\n    Int b)\n{Explains what DoThing does}\nEndFunction\n";
        let tokens = tokens_of(source);

        assert_eq!(
            documentation_comment(source, &tokens, 4).as_deref(),
            Some("Explains what DoThing does")
        );
    }

    #[test]
    fn documentation_comment_is_none_when_the_declaration_has_no_comment() {
        let source = "ScriptName Example\n\nFunction DoThing()\nEndFunction\n";
        let tokens = tokens_of(source);

        assert_eq!(documentation_comment(source, &tokens, 1), None);
        assert_eq!(documentation_comment(source, &tokens, 3), None);
    }

    #[test]
    fn documentation_comment_is_none_for_an_empty_brace_comment() {
        let source = "ScriptName Example\n{   }\n";
        let tokens = tokens_of(source);

        assert_eq!(documentation_comment(source, &tokens, 1), None);
    }
}

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
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (config, external);

    let Some(script) = ast else {
        return Vec::new();
    };
    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let lines: Vec<&str> = source.split('\n').collect();
    let mut diagnostics = Vec::new();

    check_declaration(
        script.line,
        format!("The `ScriptName {}` declaration", script.name),
        tokens,
        &lines,
        &mut diagnostics,
    );

    for property in &script.properties {
        check_declaration(
            property.line,
            format!("Property `{}`", property.name),
            tokens,
            &lines,
            &mut diagnostics,
        );
    }

    for function in all_functions(script) {
        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };
        check_declaration(
            function.line,
            format!("{kind} `{}`", function.name),
            tokens,
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
#[path = "missing_doc_comment_tests.rs"]
mod tests;

//! Flags discarded results of functions marked `; @nodiscard`.

use std::collections::HashSet;

use papyrus_parser::ast::Script;
use papyrus_parser::token::{Keyword, Token, TokenKind};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-nodiscard";

#[derive(Default)]
struct Collect {
    store: crate::visitor::Store,
}

impl crate::visitor::TokenLint for Collect {
    fn store(&mut self) -> &mut crate::visitor::Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut crate::visitor::VisitCtx<'_>) {
        self.store.extend(lint_issues(
            ctx.source,
            ctx.ast,
            ctx.tokens,
            ctx.config,
            ctx.external,
        ));
    }
}

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Shared lookup state for deciding whether a discarded call is `@nodiscard`.
struct NodiscardContext<'a> {
    ast: Option<&'a Script>,
    local: &'a HashSet<String>,
    script_name: Option<&'a str>,
    type_env: Option<&'a TypeEnv>,
}

/// Checks for calls to functions marked `; @nodiscard` whose result is
/// discarded rather than assigned, returned, or used by another expression.
/// Flagged as a `[warning]`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = config;

    let Some(tokens) = tokens else {
        return Vec::new();
    };

    let local = local_nodiscard_functions(source, tokens);
    let script_name = script_name(ast, tokens);
    let type_env = ast.map(TypeEnv::for_script);
    let context = NodiscardContext {
        ast,
        local: &local,
        script_name: script_name.as_deref(),
        type_env: type_env.as_ref(),
    };

    tokens
        .split(|token| matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .filter_map(|statement| check_statement(statement, &context, external))
        .collect()
}

/// Flags `statement` if any top-level operand of its expression is a call
/// to a `; @nodiscard` function, with no keyword or assignment that would
/// consume the statement's overall result.
fn check_statement(
    statement: &[Token],
    context: &NodiscardContext<'_>,
    external: &mut dyn ExternalSignatures,
) -> Option<Diagnostic> {
    if statement.iter().any(|token| {
        matches!(
            token.kind,
            TokenKind::Keyword(_)
                | TokenKind::Assign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::StarAssign
                | TokenKind::SlashAssign
                | TokenKind::PercentAssign
        )
    }) {
        return None;
    }

    top_level_operands(statement)
        .into_iter()
        .find_map(|operand| check_operand(operand, context, external))
}

fn check_operand(
    operand: &[Token],
    context: &NodiscardContext<'_>,
    external: &mut dyn ExternalSignatures,
) -> Option<Diagnostic> {
    let last = operand.last()?;
    if !matches!(last.kind, TokenKind::RParen) {
        return None;
    }

    let open_index = matching_open_paren(operand, operand.len() - 1)?;
    let name_index = open_index.checked_sub(1)?;
    let token = &operand[name_index];
    let TokenKind::Identifier(name) = &token.kind else {
        return None;
    };

    let qualifier = qualifier_before(operand, name_index);
    if !is_nodiscard(name, qualifier, token.line, context, external) {
        return None;
    }

    Some(Diagnostic {
        line: token.line,
        column: token.col,
        message: format!(
            "[warning] Nodiscard function '{name}' is called without using its return value"
        ),
        rule: RULE,
    })
}

fn qualifier_before(operand: &[Token], name_index: usize) -> Option<&str> {
    let dot_index = name_index.checked_sub(1)?;
    if !matches!(operand[dot_index].kind, TokenKind::Dot) {
        return None;
    }
    let qual_index = dot_index.checked_sub(1)?;
    match &operand[qual_index].kind {
        TokenKind::Identifier(name) => Some(name.as_str()),
        TokenKind::Keyword(Keyword::Self_) => Some("Self"),
        TokenKind::Keyword(Keyword::Parent) => Some("Parent"),
        _ => None,
    }
}

fn is_nodiscard(
    function_name: &str,
    qualifier: Option<&str>,
    line: usize,
    context: &NodiscardContext<'_>,
    external: &mut dyn ExternalSignatures,
) -> bool {
    let local_key = function_name.to_ascii_lowercase();
    let self_like = qualifier.is_none()
        || qualifier.is_some_and(|name| {
            name.eq_ignore_ascii_case("self") || name.eq_ignore_ascii_case("parent")
        });

    if self_like && context.local.contains(&local_key) {
        return true;
    }

    if self_like {
        if let Some(script) = context.script_name {
            if external.is_nodiscard_function(script, function_name) == Some(true) {
                return true;
            }
        }
        return false;
    }

    let Some(qualifier) = qualifier else {
        return false;
    };
    if external.is_nodiscard_function(qualifier, function_name) == Some(true) {
        return true;
    }

    resolved_qualifier_type(qualifier, context, line).is_some_and(|type_name| {
        external.is_nodiscard_function(&type_name, function_name) == Some(true)
    })
}

fn resolved_qualifier_type(
    qualifier: &str,
    context: &NodiscardContext<'_>,
    line: usize,
) -> Option<String> {
    if let Some(type_name) = context.type_env.and_then(|env| env.lookup(qualifier)) {
        return Some(type_name.name.clone());
    }

    let script = context.ast?;
    let function = containing_function(script, line)?;
    let mut env = TypeEnv::for_script(script);
    let mut found = None;
    env.with_function_scope(function, |env| {
        found = env
            .lookup(qualifier)
            .map(|type_name| type_name.name.clone());
    });
    found
}

fn containing_function(script: &Script, line: usize) -> Option<&papyrus_parser::ast::FunctionDecl> {
    all_functions(script)
        .filter(|function| function.line <= line)
        .max_by_key(|function| function.line)
}

fn all_functions(script: &Script) -> impl Iterator<Item = &papyrus_parser::ast::FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn script_name(ast: Option<&Script>, tokens: &[Token]) -> Option<String> {
    if let Some(script) = ast {
        return Some(script.name.clone());
    }
    let script_name_at = tokens
        .iter()
        .position(|token| matches!(token.kind, TokenKind::Keyword(Keyword::ScriptName)))?;
    tokens
        .get(script_name_at + 1)
        .and_then(|token| match &token.kind {
            TokenKind::Identifier(name) => Some(name.clone()),
            _ => None,
        })
}

fn local_nodiscard_functions(source: &str, tokens: &[Token]) -> HashSet<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut names = HashSet::new();
    let mut index = 0;
    while index < tokens.len() {
        if matches!(tokens[index].kind, TokenKind::Keyword(Keyword::Function)) {
            if let Some(name) = tokens.get(index + 1).and_then(|token| match &token.kind {
                TokenKind::Identifier(name) => Some(name.as_str()),
                _ => None,
            }) {
                if header_has_nodiscard(&lines, tokens, tokens[index].line) {
                    names.insert(name.to_ascii_lowercase());
                }
            }
        }
        index += 1;
    }
    names
}

fn header_has_nodiscard(lines: &[&str], tokens: &[Token], line: usize) -> bool {
    if line == 0 {
        return false;
    }
    let last = tokens
        .iter()
        .find(|token| token.kind == TokenKind::Newline && token.line >= line)
        .map(|token| token.line)
        .unwrap_or(line);
    let start = line.saturating_sub(2);
    lines
        .get(start..last.min(lines.len()))
        .is_some_and(|slice| slice.iter().any(|row| line_has_nodiscard(row)))
}

fn line_has_nodiscard(line: &str) -> bool {
    let Some(comment) = line_comment_text(line) else {
        return false;
    };
    let lowered = comment.to_ascii_lowercase();
    let Some(index) = lowered.find("@nodiscard") else {
        return false;
    };
    let before_ok = index == 0
        || lowered[..index]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_whitespace() || c == ',');
    let after = &lowered[index + "@nodiscard".len()..];
    let after_ok = after
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || c == ',');
    before_ok && after_ok
}

fn line_comment_text(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => index += 1,
            b';' if !in_string => {
                return if bytes.get(index + 1) == Some(&b'/') {
                    None
                } else {
                    Some(line[index + 1..].trim())
                };
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn top_level_operands(tokens: &[Token]) -> Vec<&[Token]> {
    let mut operands = Vec::new();
    let mut start = 0;
    let mut paren_depth: usize = 0;
    let mut bracket_depth: usize = 0;

    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Gt
            | TokenKind::Lt
            | TokenKind::GtEq
            | TokenKind::LtEq
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Not
                if paren_depth == 0 && bracket_depth == 0 =>
            {
                if index > start {
                    operands.push(&tokens[start..index]);
                }
                start = index + 1;
            }
            _ => {}
        }
    }

    if start < tokens.len() {
        operands.push(&tokens[start..]);
    }

    operands
}

fn matching_open_paren(tokens: &[Token], close_index: usize) -> Option<usize> {
    let mut depth = 0;
    for index in (0..=close_index).rev() {
        match tokens[index].kind {
            TokenKind::RParen => depth += 1,
            TokenKind::LParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "unused_nodiscard_tests.rs"]
mod tests;

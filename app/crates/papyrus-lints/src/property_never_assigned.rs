//! Flags a non-`Auto` property whose backing value is never written.
//!
//! Full property blocks (`Get`/`Set` between `Property` and `EndProperty`)
//! can be read through the property name while the value they expose stays
//! at Papyrus's implicit default forever. `unused-property` only cares that
//! the property *name* is referenced; this rule looks for an actual write
//! of that name or of the field the accessors read and write.
//!
//! Disabled by default: Creation Kit–filled and otherwise externally set
//! properties are common. Suppress a single property with `; @external` on
//! its declaration line.

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::comment_annotations::parse_line_annotations;
use papyrus_parser::token::{Keyword, Token, TokenKind};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "property-never-assigned";

#[derive(Default)]
struct Collect {
    store: Store,
    decls: Vec<PropertyInfo>,
    writes: Vec<(String, usize)>,
    lines: Vec<String>,
}

struct PropertyInfo {
    lower: String,
    name: String,
    name_index: usize,
    line: usize,
    column: usize,
    body_start: usize,
    body_end: usize,
    external: bool,
    backing: Vec<String>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.lines = ctx.source.split('\n').map(str::to_string).collect();
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        if is_assignment_target(tokens, index) {
            if let TokenKind::Identifier(name) = &token.kind {
                self.writes.push((name.to_ascii_lowercase(), index));
            }
        }

        if !matches!(token.kind, TokenKind::Keyword(Keyword::Property)) {
            return;
        }
        if !preceded_by_type_name(tokens, index) {
            return;
        }
        let Some(name_token) = tokens.get(index + 1) else {
            return;
        };
        let TokenKind::Identifier(name) = &name_token.kind else {
            return;
        };

        let header_end = header_end_index(tokens, index + 2);
        if header_is_auto(tokens, index + 2, header_end) {
            return;
        }

        let body_start = header_end + 1;
        let body_end = matching_end_property(tokens, body_start).unwrap_or(tokens.len());
        let backing = backing_fields(tokens, body_start, body_end);
        let external = line_has_external(&self.lines, name_token.line);

        self.decls.push(PropertyInfo {
            lower: name.to_ascii_lowercase(),
            name: name.clone(),
            name_index: index + 1,
            line: name_token.line,
            column: name_token.col,
            body_start,
            body_end,
            external,
            backing,
        });
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for decl in &self.decls {
            if decl.external {
                continue;
            }
            if decl.backing.is_empty() {
                continue;
            }
            let written = self.writes.iter().any(|(candidate, write_index)| {
                if *write_index == decl.name_index {
                    return false;
                }
                if *write_index >= decl.body_start && *write_index < decl.body_end {
                    return false;
                }
                candidate == &decl.lower || decl.backing.iter().any(|field| field == candidate)
            });
            if written {
                continue;
            }
            self.store.emit(
                decl.line,
                decl.column,
                format!(
                    "[warning] Property '{}' is never assigned; readers stay on the default value",
                    decl.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

/// Checks `source` for non-`Auto` properties whose backing field and
/// property name are never written outside the property block.
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

fn preceded_by_type_name(tokens: &[Token], property_index: usize) -> bool {
    if property_index == 0 {
        return false;
    }
    if matches!(tokens[property_index - 1].kind, TokenKind::Identifier(_)) {
        return true;
    }
    property_index >= 3
        && matches!(tokens[property_index - 1].kind, TokenKind::RBracket)
        && matches!(tokens[property_index - 2].kind, TokenKind::LBracket)
        && matches!(tokens[property_index - 3].kind, TokenKind::Identifier(_))
}

fn header_end_index(tokens: &[Token], start: usize) -> usize {
    let mut index = start;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::Newline | TokenKind::Eof | TokenKind::Keyword(Keyword::EndProperty) => {
                return index;
            }
            TokenKind::Keyword(Keyword::Function) => return index.saturating_sub(1),
            _ => index += 1,
        }
    }
    tokens.len().saturating_sub(1)
}

fn header_is_auto(tokens: &[Token], start: usize, end: usize) -> bool {
    tokens[start..=end.min(tokens.len().saturating_sub(1))]
        .iter()
        .any(|token| {
            matches!(
                token.kind,
                TokenKind::Keyword(Keyword::Auto) | TokenKind::Keyword(Keyword::AutoReadOnly)
            )
        })
}

fn line_has_external(lines: &[String], line: usize) -> bool {
    let Some(index) = line.checked_sub(1) else {
        return false;
    };
    lines.get(index).is_some_and(|source_line| {
        parse_line_annotations(source_line)
            .iter()
            .any(|annotation| annotation.name.eq_ignore_ascii_case("external"))
    })
}

fn matching_end_property(tokens: &[Token], start: usize) -> Option<usize> {
    tokens[start..]
        .iter()
        .position(|token| matches!(token.kind, TokenKind::Keyword(Keyword::EndProperty)))
        .map(|offset| start + offset)
}

fn is_assignment_op(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Assign
            | TokenKind::PlusAssign
            | TokenKind::MinusAssign
            | TokenKind::StarAssign
            | TokenKind::SlashAssign
            | TokenKind::PercentAssign
    )
}

fn is_assignment_target(tokens: &[Token], index: usize) -> bool {
    let TokenKind::Identifier(_) = tokens[index].kind else {
        return false;
    };
    if index > 0 && matches!(tokens[index - 1].kind, TokenKind::Dot) {
        return matches!(
            tokens.get(index.saturating_sub(2)).map(|token| &token.kind),
            Some(TokenKind::Keyword(Keyword::Self_))
        );
    }
    tokens
        .get(index + 1)
        .is_some_and(|next| is_assignment_op(&next.kind))
}

fn backing_fields(tokens: &[Token], body_start: usize, body_end: usize) -> Vec<String> {
    let mut fields = Vec::new();
    let mut index = body_start;
    while index < body_end {
        if matches!(tokens[index].kind, TokenKind::Keyword(Keyword::Function)) {
            let (fn_end, params) = function_span(tokens, index, body_end);
            collect_backing_from_function(tokens, index, fn_end, &params, &mut fields);
            index = fn_end + 1;
            continue;
        }
        index += 1;
    }
    fields
}

fn function_span(tokens: &[Token], start: usize, limit: usize) -> (usize, Vec<String>) {
    let mut params = Vec::new();
    let mut index = start + 1;
    while index < limit {
        match &tokens[index].kind {
            TokenKind::Keyword(Keyword::EndFunction) => return (index, params),
            TokenKind::Identifier(_) => {
                if tokens
                    .get(index + 1)
                    .is_some_and(|next| matches!(next.kind, TokenKind::Identifier(_)))
                {
                    if let TokenKind::Identifier(param) = &tokens[index + 1].kind {
                        params.push(param.to_ascii_lowercase());
                    }
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    (limit, params)
}

fn is_simple_returned_identifier(tokens: &[Token], index: usize) -> bool {
    if index == 0 || !matches!(tokens[index - 1].kind, TokenKind::Keyword(Keyword::Return)) {
        return false;
    }
    !matches!(
        tokens.get(index + 1).map(|token| &token.kind),
        Some(TokenKind::Dot | TokenKind::LParen | TokenKind::LBracket)
    )
}

fn collect_backing_from_function(
    tokens: &[Token],
    start: usize,
    end: usize,
    params: &[String],
    fields: &mut Vec<String>,
) {
    let mut index = start;
    while index < end {
        if let TokenKind::Identifier(name) = &tokens[index].kind {
            let lower = name.to_ascii_lowercase();
            if params.iter().any(|param| param == &lower) {
                index += 1;
                continue;
            }
            let assigned = is_assignment_target(tokens, index);
            let returned = is_simple_returned_identifier(tokens, index);
            if (assigned || returned) && !fields.iter().any(|existing| existing == &lower) {
                fields.push(lower);
            }
        }
        index += 1;
    }
}

#[cfg(test)]
#[path = "property_never_assigned_tests.rs"]
mod tests;

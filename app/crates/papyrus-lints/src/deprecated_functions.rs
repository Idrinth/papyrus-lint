//! Flags calls to functions declared with `; @deprecated` or marked as
//! deprecated in an externally resolved saved AST. Token-based matching also
//! lets the rule run when the source does not produce a complete AST.

use std::collections::HashSet;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::ast::{FunctionDecl, Script};
use papyrus_parser::token::{Keyword, Token, TokenKind};
use papyrus_parser::types::TypeEnv;

pub const RULE: &str = "deprecated-functions";

#[derive(Default)]
struct Collect {
    store: Store,
    local: HashSet<String>,
    script_name: Option<String>,
    env: Option<TypeEnv>,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.script_name = ctx.ast.map(|script| script.name.clone());
        self.env = ctx.ast.map(TypeEnv::for_script);
        self.local.clear();
        if let Some(script) = ctx.ast {
            self.local.extend(
                all_functions(script)
                    .filter(|function| function.deprecated)
                    .map(|function| function.name.to_ascii_lowercase()),
            );
        }
        let Some(tokens) = ctx.tokens else {
            return;
        };
        let lines: Vec<&str> = ctx.source.lines().collect();
        for (index, token) in tokens.iter().enumerate() {
            if !matches!(token.kind, TokenKind::Keyword(Keyword::Function)) {
                continue;
            }
            let Some(TokenKind::Identifier(name)) =
                tokens.get(index + 1).map(|token| &token.kind)
            else {
                continue;
            };
            if header_has_deprecated(&lines, tokens, token.line) {
                self.local.insert(name.to_ascii_lowercase());
            }
        }
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        ctx: &mut VisitCtx<'_>,
    ) {
        let TokenKind::Identifier(name) = &token.kind else {
            return;
        };
        if !matches!(tokens.get(index + 1).map(|token| &token.kind), Some(TokenKind::LParen)) {
            return;
        }
        // A deprecation marker applies to callers, not the declaration itself.
        if matches!(
            tokens.get(index.wrapping_sub(1)).map(|token| &token.kind),
            Some(TokenKind::Keyword(Keyword::Function))
        ) {
            return;
        }

        let qualifier = qualifier_before(tokens, index);
        let context = DeprecatedContext {
            ast: ctx.ast,
            local: &self.local,
            script_name: self.script_name.as_deref(),
            type_env: self.env.as_ref(),
        };
        if is_deprecated(name, qualifier, token.line, &context, ctx.external) {
            self.store.emit(
                token.line,
                token.col,
                format!("[warning] Function '{name}' is marked deprecated"),
                RULE,
            );
        }
    }
}

struct DeprecatedContext<'a> {
    ast: Option<&'a Script>,
    local: &'a HashSet<String>,
    script_name: Option<&'a str>,
    type_env: Option<&'a TypeEnv>,
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn qualifier_before(tokens: &[Token], call_index: usize) -> Option<&str> {
    let dot_index = call_index.checked_sub(1)?;
    if !matches!(tokens[dot_index].kind, TokenKind::Dot) {
        return None;
    }
    let qualifier_index = dot_index.checked_sub(1)?;
    match &tokens[qualifier_index].kind {
        TokenKind::Identifier(name) => Some(name),
        TokenKind::Keyword(Keyword::Self_) => Some("Self"),
        TokenKind::Keyword(Keyword::Parent) => Some("Parent"),
        _ => None,
    }
}

fn is_deprecated(
    function_name: &str,
    qualifier: Option<&str>,
    line: usize,
    context: &DeprecatedContext<'_>,
    external: &mut dyn ExternalSignatures,
) -> bool {
    let self_like = qualifier.is_none()
        || qualifier.is_some_and(|name| {
            name.eq_ignore_ascii_case("self") || name.eq_ignore_ascii_case("parent")
        });
    if self_like && context.local.contains(&function_name.to_ascii_lowercase()) {
        return true;
    }
    if self_like {
        return context.script_name.is_some_and(|script| {
            external.is_deprecated_function(script, function_name) == Some(true)
        });
    }

    let Some(qualifier) = qualifier else {
        return false;
    };
    if external.is_deprecated_function(qualifier, function_name) == Some(true) {
        return true;
    }
    resolved_qualifier_type(qualifier, context, line).is_some_and(|type_name| {
        external.is_deprecated_function(&type_name, function_name) == Some(true)
    })
}

fn resolved_qualifier_type(
    qualifier: &str,
    context: &DeprecatedContext<'_>,
    line: usize,
) -> Option<String> {
    if let Some(type_name) = context.type_env.and_then(|env| env.lookup(qualifier)) {
        return Some(type_name.name.clone());
    }
    let script = context.ast?;
    let function = all_functions(script)
        .filter(|function| function.line <= line)
        .max_by_key(|function| function.line)?;
    let mut env = TypeEnv::for_script(script);
    let mut found = None;
    env.with_function_scope(function, |env| {
        found = env
            .lookup(qualifier)
            .map(|type_name| type_name.name.clone());
    });
    found
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn header_has_deprecated(lines: &[&str], tokens: &[Token], line: usize) -> bool {
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
        .is_some_and(|slice| slice.iter().any(|row| line_has_deprecated(row)))
}

fn line_has_deprecated(line: &str) -> bool {
    papyrus_parser::comment_annotations::parse_line_annotations(line)
        .iter()
        .any(|annotation| annotation.name.eq_ignore_ascii_case("deprecated"))
}

#[cfg(test)]
#[path = "deprecated_functions_tests.rs"]
mod tests;

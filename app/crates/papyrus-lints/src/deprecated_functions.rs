//! Flags calls to functions listed in `shared/rules/data/deprecated-functions.yaml`
//! or declared with `; @deprecated`.
//!
//! The data is compiled into `DEPRECATED_FUNCTIONS` by `build.rs`, so the
//! linter does not parse YAML at runtime. Token-based matching also lets the
//! rule run when the source does not produce a complete AST. Bundled AST
//! enrichment and project directives share the same follow-up lookup path.

use std::collections::{HashMap, HashSet};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::ast::{Deprecation, FunctionDecl, Script};
use papyrus_parser::token::{Keyword, Token, TokenKind};
use papyrus_parser::types::TypeEnv;

pub struct DeprecatedFunctionRule {
    pub script: &'static str,
    pub function: &'static str,
    #[allow(dead_code)]
    pub replacement: Option<&'static str>,
    pub level: &'static str,
    pub message: &'static str,
    /// Whether `script` is a native singleton that must be called through
    /// its literal script name rather than through an object instance.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/deprecated_functions_data.rs"));

pub const RULE: &str = "deprecated-functions";

#[derive(Default)]
struct Collect {
    store: Store,
    local: HashSet<String>,
    /// Replacement/guidance text captured from a local `; @deprecated <note>`
    /// annotation, keyed by lowercased function name. A present key with a
    /// `None` value means the function is annotated but the annotation
    /// carries no note text.
    local_notes: HashMap<String, Option<String>>,
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
        self.local_notes.clear();
        if let Some(script) = ctx.ast {
            self.local.extend(
                all_functions(script)
                    .filter(|function| function.deprecation.is_some())
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
            if let Some(note) = header_deprecated_note(&lines, tokens, token.line) {
                let key = name.to_ascii_lowercase();
                self.local.insert(key.clone());
                self.local_notes.insert(key, note);
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
        if let Some(rule) = find_rule(name) {
            if !rule.global || qualifier_matches(tokens, index, rule.script) {
                self.store.emit(
                    token.line,
                    token.col,
                    format!(
                        "[{}] {}.{}: {}",
                        rule.level, rule.script, rule.function, rule.message
                    ),
                    RULE,
                );
                return;
            }
        }
        // A project directive marks callers, not the declaration itself.
        // Keep this after the compiled-rule lookup: declarations in the
        // bundled API scripts have historically been reported by that data.
        if matches!(
            tokens.get(index.wrapping_sub(1)).map(|token| &token.kind),
            Some(TokenKind::Keyword(Keyword::Function))
        ) {
            return;
        }

        // `None` here means a dot precedes the call but its receiver isn't a
        // plain identifier/Self/Parent (e.g. a chained call's return value):
        // that's not resolvable to a script name, so don't guess it's a
        // same-script ("self-like") call either.
        let Some(qualifier) = qualifier_before(tokens, index) else {
            return;
        };
        let context = DeprecatedContext {
            ast: ctx.ast,
            local: &self.local,
            local_notes: &self.local_notes,
            script_name: self.script_name.as_deref(),
            type_env: self.env.as_ref(),
        };
        if let Some(deprecation) = deprecation(name, qualifier, token.line, &context, ctx.external) {
            self.store.emit(
                token.line,
                token.col,
                format!("[{}] {}", deprecation.level, deprecation.message),
                RULE,
            );
        }
    }
}

struct DeprecatedContext<'a> {
    ast: Option<&'a Script>,
    local: &'a HashSet<String>,
    local_notes: &'a HashMap<String, Option<String>>,
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

fn qualifier_matches(tokens: &[Token], call_index: usize, script: &str) -> bool {
    if call_index < 2 || !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(script)
}

/// The qualifier preceding a call, distinguishing an unqualified call
/// (`Some(None)`) from one whose receiver can't be resolved to a name
/// (`None`, e.g. a chained call's return value) from a plain qualifier
/// (`Some(Some(name))`).
fn qualifier_before(tokens: &[Token], call_index: usize) -> Option<Option<&str>> {
    let Some(dot_index) = call_index.checked_sub(1) else {
        return Some(None);
    };
    if !matches!(tokens[dot_index].kind, TokenKind::Dot) {
        return Some(None);
    }
    let qualifier_index = dot_index.checked_sub(1)?;
    match &tokens[qualifier_index].kind {
        TokenKind::Identifier(name) => Some(Some(name)),
        TokenKind::Keyword(Keyword::Self_) => Some(Some("Self")),
        TokenKind::Keyword(Keyword::Parent) => Some(Some("Parent")),
        _ => None,
    }
}

fn deprecation(
    function_name: &str,
    qualifier: Option<&str>,
    line: usize,
    context: &DeprecatedContext<'_>,
    external: &mut dyn ExternalSignatures,
) -> Option<Deprecation> {
    let self_like = qualifier.is_none()
        || qualifier.is_some_and(|name| {
            name.eq_ignore_ascii_case("self") || name.eq_ignore_ascii_case("parent")
        });
    if self_like && context.local.contains(&function_name.to_ascii_lowercase()) {
        if let Some(script) = context.ast {
            if let Some(found) = all_functions(script).find(|function| {
                function.name.eq_ignore_ascii_case(function_name) && function.deprecation.is_some()
            }) {
                return found.deprecation.clone();
            }
        }
        let note = context
            .local_notes
            .get(&function_name.to_ascii_lowercase())
            .cloned()
            .flatten();
        return Some(generic_deprecation(function_name, note));
    }
    if self_like {
        return context
            .script_name
            .and_then(|script| external.deprecated_function(script, function_name));
    }

    let qualifier = qualifier?;
    if let Some(deprecation) = external.deprecated_function(qualifier, function_name) {
        return Some(deprecation);
    }
    resolved_qualifier_type(qualifier, context, line)
        .and_then(|type_name| external.deprecated_function(&type_name, function_name))
}

fn generic_deprecation(function_name: &str, note: Option<String>) -> Deprecation {
    let message = match note {
        Some(note) => format!("Function '{function_name}' is marked deprecated: {note}"),
        None => format!("Function '{function_name}' is marked deprecated"),
    };
    Deprecation {
        replacement: None,
        level: "warning".to_string(),
        message,
    }
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

/// Whether the function header starting at `line` (or the line above it) is
/// marked with `; @deprecated`, and the note text following it, if any.
/// Returns `None` when no `@deprecated` annotation is present at all.
fn header_deprecated_note(lines: &[&str], tokens: &[Token], line: usize) -> Option<Option<String>> {
    if line == 0 {
        return None;
    }
    let last = tokens
        .iter()
        .find(|token| token.kind == TokenKind::Newline && token.line >= line)
        .map(|token| token.line)
        .unwrap_or(line);
    let start = line.saturating_sub(2);
    lines
        .get(start..last.min(lines.len()))?
        .iter()
        .find_map(|row| deprecated_note(row))
}

fn deprecated_note(line: &str) -> Option<Option<String>> {
    papyrus_parser::comment_annotations::parse_line_annotations(line)
        .into_iter()
        .find(|annotation| annotation.name.eq_ignore_ascii_case("deprecated"))
        .map(|annotation| {
            let note = annotation.arguments.trim();
            (!note.is_empty()).then(|| note.to_string())
        })
}

#[cfg(test)]
fn line_has_deprecated(line: &str) -> bool {
    deprecated_note(line).is_some()
}

fn find_rule(name: &str) -> Option<&'static DeprecatedFunctionRule> {
    DEPRECATED_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "deprecated_functions_tests.rs"]
mod tests;

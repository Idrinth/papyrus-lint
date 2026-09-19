//! Flags a declared identifier (a function/event, property, state,
//! parameter, or local/script variable) whose name doesn't match the
//! project's configured casing style (see [`crate::config::IdentifierCasing`]).
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a declaration's name apart from any other identifier
//! in the script; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. `ScriptName` itself is never checked, since it
//! must match the script's filename regardless of casing style.
//!
//! A parameter has no line of its own in the AST, so it's reported on its
//! enclosing function's line.

use std::collections::HashMap;

use papyrus_parser::ast::{FunctionDecl, Param, PropertyDecl, StateDecl, Stmt, VariableDecl};
use papyrus_parser::token::{Keyword, TokenKind};

use crate::config::IdentifierCasing;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "identifier-casing";

#[derive(Default)]
struct Collect {
    store: Store,
    protected: Vec<bool>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
    }

    fn visit_property(&mut self, property: &PropertyDecl, ctx: &mut VisitCtx<'_>) {
        check_name(
            &property.name,
            "Property",
            property.line,
            ctx.config.identifier_casing,
            &self.protected,
            &mut self.store,
        );
    }

    fn visit_variable(&mut self, variable: &VariableDecl, ctx: &mut VisitCtx<'_>) {
        check_name(
            &variable.name,
            "Variable",
            variable.line,
            ctx.config.identifier_casing,
            &self.protected,
            &mut self.store,
        );
    }

    fn visit_state(&mut self, state: &StateDecl, ctx: &mut VisitCtx<'_>) {
        check_name(
            &state.name,
            "State",
            state.line,
            ctx.config.identifier_casing,
            &self.protected,
            &mut self.store,
        );
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };
        check_name(
            &function.name,
            kind,
            function.line,
            ctx.config.identifier_casing,
            &self.protected,
            &mut self.store,
        );
    }

    fn visit_param(&mut self, param: &Param, ctx: &mut VisitCtx<'_>) {
        check_name(
            &param.name,
            "Parameter",
            ctx.line,
            ctx.config.identifier_casing,
            &self.protected,
            &mut self.store,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for declared identifiers that don't conform to `style`.
/// Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged: it's CreationKit-generated boilerplate the user can't
/// edit or rename.
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

/// Renames every non-conforming declaration and its references to `style`
/// when doing so does not add, remove, or move underscores.
///
/// Papyrus identifiers are case-insensitive, so references are matched that
/// way too. Tokens in comments and strings are naturally excluded by the
/// lexer, and CreationKit-owned fragment wrapper lines are left untouched.
/// If the source cannot be parsed or tokenized, it is returned unchanged.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let style = config.identifier_casing;

    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let protected = fragment_code::protected_lines(source);
    let mut renames = HashMap::new();

    for variable in &script.variables {
        collect_name(
            &variable.name,
            variable.line,
            style,
            &protected,
            &mut renames,
        );
    }
    for property in &script.properties {
        collect_name(
            &property.name,
            property.line,
            style,
            &protected,
            &mut renames,
        );
    }
    for state in &script.states {
        collect_state_names(state, style, &protected, &mut renames);
    }
    for function in &script.functions {
        collect_function_names(function, style, &protected, &mut renames);
    }
    if renames.is_empty() {
        return source.to_string();
    }

    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let line_offsets = line_offsets(source);
    let mut follows_script_name = false;

    for token in tokens {
        match token.kind {
            TokenKind::Keyword(Keyword::ScriptName) => follows_script_name = true,
            TokenKind::Identifier(name) => {
                if follows_script_name {
                    follows_script_name = false;
                    continue;
                }
                if protected.get(token.line).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(replacement) = renames.get(&name.to_ascii_lowercase()) {
                    let start = line_offsets[token.line - 1] + token.col - 1;
                    replacements.push((start, start + name.len(), replacement.clone()));
                }
            }
            TokenKind::Newline => follows_script_name = false,
            _ => {}
        }
    }

    let mut repaired = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

fn collect_state_names(
    state: &StateDecl,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    collect_name(&state.name, state.line, style, protected, renames);
    for function in &state.functions {
        collect_function_names(function, style, protected, renames);
    }
}

fn collect_function_names(
    function: &FunctionDecl,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    collect_name(&function.name, function.line, style, protected, renames);
    for param in &function.params {
        collect_name(&param.name, function.line, style, protected, renames);
    }
    for stmt in &function.body {
        collect_stmt_names(stmt, style, protected, renames);
    }
}

fn collect_stmt_names(
    stmt: &Stmt,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    match stmt {
        Stmt::VarDecl(decl) => collect_name(&decl.name, decl.line, style, protected, renames),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for stmt in branches
                .iter()
                .flat_map(|branch| &branch.body)
                .chain(else_body)
            {
                collect_stmt_names(stmt, style, protected, renames);
            }
        }
        Stmt::While { body, .. } => {
            for stmt in body {
                collect_stmt_names(stmt, style, protected, renames);
            }
        }
        Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
    }
}

fn collect_name(
    name: &str,
    line: usize,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    if !protected.get(line).copied().unwrap_or(false) && !style.matches(name) {
        let replacement = convert_name(name, style);
        // Underscores are often a meaningful, externally-visible part of a
        // Papyrus identifier.  In particular, removing one can turn a name
        // such as `DFO_VampireFeed` into a different identifier rather than
        // merely correcting its letter case. Leave these substantive renames
        // to the user and only apply an automatic fix when every underscore
        // remains in exactly the same position.
        if underscore_offsets(name).eq(underscore_offsets(&replacement)) {
            renames.insert(name.to_ascii_lowercase(), replacement);
        }
    }
}

fn underscore_offsets(name: &str) -> impl Iterator<Item = usize> + '_ {
    name.match_indices('_').map(|(offset, _)| offset)
}

fn convert_name(name: &str, style: IdentifierCasing) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut start = 0;
    for i in 0..=chars.len() {
        let boundary = i == chars.len()
            || chars[i] == '_'
            || (i > start && chars[i].is_ascii_uppercase() && chars[i - 1].is_ascii_lowercase())
            || (i > start
                && i + 1 < chars.len()
                && chars[i - 1].is_ascii_uppercase()
                && chars[i].is_ascii_uppercase()
                && chars[i + 1].is_ascii_lowercase());
        if boundary {
            if start < i {
                words.push(
                    chars[start..i]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase(),
                );
            }
            start = i + usize::from(i < chars.len() && chars[i] == '_');
        }
    }
    match style {
        IdentifierCasing::SnakeCase => words.join("_"),
        IdentifierCasing::ConstantCase => words.join("_").to_ascii_uppercase(),
        IdentifierCasing::CamelCase | IdentifierCasing::PascalCase => words
            .into_iter()
            .enumerate()
            .map(|(index, word)| {
                if index == 0 && style == IdentifierCasing::CamelCase {
                    word
                } else {
                    let mut chars = word.chars();
                    chars
                        .next()
                        .map(|c| c.to_ascii_uppercase())
                        .into_iter()
                        .chain(chars)
                        .collect()
                }
            })
            .collect(),
    }
}

fn line_offsets(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(source.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

fn check_name(
    name: &str,
    kind: &str,
    line: usize,
    style: IdentifierCasing,
    protected: &[bool],
    store: &mut Store,
) {
    if protected.get(line).copied().unwrap_or(false) {
        return;
    }
    if style.matches(name) {
        return;
    }

    store.emit(
        line,
        1,
        format!(
            "[warning] {kind} '{name}' does not match the configured {} casing style",
            style.label()
        ),
        RULE,
    );
}

#[cfg(test)]
#[path = "identifier_casing_tests.rs"]
mod tests;

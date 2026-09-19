//! Flags an assignment that writes to a script-level property declared
//! `AutoReadOnly` (e.g. `Float Property a = 0.1 AutoReadOnly`), since
//! Papyrus rejects such an assignment at compile time — an `AutoReadOnly`
//! property can only ever be set to its declared initial value.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell an `AutoReadOnly` property's own name apart from an unrelated
//! local variable or parameter that happens to share it; a script that
//! doesn't parse cleanly is left unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt, VariableDecl};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "readonly-property-write";

#[derive(Default)]
struct Collect {
    store: Store,
    readonly: HashSet<String>,
    shadowed: HashSet<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.readonly = script
            .properties
            .iter()
            .filter(|property| property.is_auto_read_only)
            .map(|property| property.name.to_ascii_lowercase())
            .collect();
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.shadowed = function
            .params
            .iter()
            .map(|param| param.name.to_ascii_lowercase())
            .chain(
                collect_var_decls(&function.body)
                    .into_iter()
                    .map(|decl| decl.name.to_ascii_lowercase()),
            )
            .collect();
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.shadowed.clear();
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        if self.readonly.is_empty() {
            return;
        }
        let Stmt::Assign { target, line, .. } = stmt else {
            return;
        };
        let Some(target) = target_property(target) else {
            return;
        };
        let name_lower = target.name.to_ascii_lowercase();
        if !self.readonly.contains(&name_lower) {
            return;
        }
        if !target.is_self_qualified && self.shadowed.contains(&name_lower) {
            return;
        }
        self.store.emit(
            *line,
            1,
            format!(
                "[error] '{}' is declared AutoReadOnly and cannot be assigned a new value",
                target.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for an assignment (`=`, `+=`, `-=`, ...) targeting a
/// script-level property declared `AutoReadOnly`, either by its bare name
/// or as `Self.PropertyName`. A bare name shadowed by a same-named local
/// variable or parameter in the enclosing function refers to that local/
/// parameter instead, and is never flagged. Flagged as an `[error]`, since
/// Papyrus rejects the assignment at compile time.
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

/// A property an assignment's target expression resolves to, either by its
/// bare name or as `Self.PropertyName`.
struct PropertyTarget<'a> {
    name: &'a str,
    is_self_qualified: bool,
}

/// If `target` is a bare identifier or a `Self.PropertyName` member access,
/// returns the referenced name. Anything else (a member access on
/// something other than `Self`, an index, ...) returns `None` rather than
/// being guessed at.
fn target_property(target: &Expr) -> Option<PropertyTarget<'_>> {
    match target {
        Expr::Identifier(name) => Some(PropertyTarget {
            name,
            is_self_qualified: false,
        }),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(PropertyTarget {
                name: property,
                is_self_qualified: true,
            })
        }
        _ => None,
    }
}

/// Finds every `VariableDecl` in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies, since Papyrus locals
/// aren't block-scoped.
fn collect_var_decls(body: &[Stmt]) -> Vec<&VariableDecl> {
    let mut decls = Vec::new();
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => decls.push(decl),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    decls.extend(collect_var_decls(&branch.body));
                }
                decls.extend(collect_var_decls(else_body));
            }
            Stmt::While { body, .. } => decls.extend(collect_var_decls(body)),
            _ => {}
        }
    }
    decls
}

#[cfg(test)]
#[path = "readonly_property_write_tests.rs"]
mod tests;

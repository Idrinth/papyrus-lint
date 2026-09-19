//! Flags a numeric literal used directly in an expression rather than
//! through a named constant, property, or local variable, as a
//! `[warning]`. Disabled by default (see [`crate::config::Rules::magic_numbers`]):
//! many existing scripts contain plenty of unremarkable literal numbers,
//! so a project has to opt in explicitly.
//!
//! `-1`, `0`, and `1` are never flagged, since they're near-universally
//! used directly (array bounds, increments, sentinel/empty checks) without
//! losing any clarity from being spelled out.
//!
//! A literal that's the entire value given to a declaration or assignment
//! (`Int kMaxTargets = 5`, later reassigned as `kMaxTargets = 6`) is left
//! alone too: naming it there already gives it the meaning this lint is
//! after. Only the bare literal itself is exempted this way; a literal
//! nested inside a more complex initializer (`Int kMaxTargets = 5 + 1`) is
//! still checked, since the declaration's name doesn't explain what either
//! operand means on its own.
//!
//! By default (the "loose" [`MagicNumbers`] mode), a numeric literal passed
//! as an argument to `Utility.Wait`, `RegisterForUpdate`,
//! `RegisterForSingleUpdate`, `RegisterForUpdateGameTime`, or
//! `RegisterForSingleUpdateGameTime` (see [`crate::short_wait_interval`],
//! which matches the same calls) is left unflagged too, since a hardcoded
//! interval there is both common and usually self-explanatory. The
//! "strict" mode also checks those arguments like any other.

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, Literal, Param, PropertyDecl, Stmt, UnaryOp, VariableDecl};
use serde::{Deserialize, Serialize};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "magic-numbers";

/// Whether this lint also checks the interval argument of a
/// `Utility.Wait`/`RegisterForUpdate`/`RegisterForSingleUpdate`/
/// `RegisterForUpdateGameTime`/`RegisterForSingleUpdateGameTime` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MagicNumbers {
    /// The interval argument of a `Utility.Wait`/`RegisterFor*` call is
    /// never flagged.
    #[default]
    Loose,
    /// Every numeric literal is checked, including a `Utility.Wait`/
    /// `RegisterFor*` call's interval argument.
    Strict,
}

#[derive(Default)]
struct Collect {
    store: Store,
    ignore: HashSet<*const Expr>,
    pending_neg: bool,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_property(&mut self, property: &PropertyDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(value) = &property.value {
            mark_tree(value, &mut self.ignore);
        }
    }

    fn visit_param(&mut self, param: &Param, _ctx: &mut VisitCtx<'_>) {
        if let Some(default) = &param.default {
            mark_tree(default, &mut self.ignore);
        }
    }

    fn visit_variable(&mut self, variable: &VariableDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(value) = &variable.value {
            if is_bare_number_literal(value) {
                mark_bare_literal(value, &mut self.ignore);
            }
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::Assign { value, .. } = stmt else {
            return;
        };
        if is_bare_number_literal(value) {
            mark_bare_literal(value, &mut self.ignore);
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        if ctx.config.magic_numbers == MagicNumbers::Loose {
            if let Expr::Call { callee, args, .. } = expr {
                if crate::short_wait_interval::matching_function(callee).is_some() {
                    for arg in args {
                        let value = match arg {
                            Expr::NamedArg { value, .. } => value.as_ref(),
                            other => other,
                        };
                        mark_wait_exempt(value, &mut self.ignore);
                    }
                }
            }
        }

        let ignored = self.ignore.contains(&(expr as *const Expr));
        match expr {
            Expr::Unary {
                op: UnaryOp::Neg,
                operand,
            } if is_number_literal(operand) => {
                if !ignored {
                    self.pending_neg = true;
                }
            }
            Expr::Literal(Literal::Int { value, .. }) => {
                if !ignored {
                    let value = if self.pending_neg { -*value } else { *value };
                    flag_int(value, ctx.line, &mut self.store);
                }
                self.pending_neg = false;
            }
            Expr::Literal(Literal::Float(f)) => {
                if !ignored {
                    let value = if self.pending_neg { -*f } else { *f };
                    flag_float(value, ctx.line, &mut self.store);
                }
                self.pending_neg = false;
            }
            _ => self.pending_neg = false,
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for numeric literals used directly rather than through
/// a named constant, property, or local variable, per `mode`.
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

fn is_bare_number_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(Literal::Int { .. } | Literal::Float(_)) => true,
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => is_number_literal(operand),
        _ => false,
    }
}

fn is_number_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Literal(Literal::Int { .. } | Literal::Float(_))
    )
}

fn mark_bare_literal(expr: &Expr, ignore: &mut HashSet<*const Expr>) {
    ignore.insert(expr as *const Expr);
    if let Expr::Unary { operand, .. } = expr {
        ignore.insert(operand.as_ref() as *const Expr);
    }
}

fn mark_tree(expr: &Expr, ignore: &mut HashSet<*const Expr>) {
    ignore.insert(expr as *const Expr);
    match expr {
        Expr::Binary { left, right, .. } => {
            mark_tree(left, ignore);
            mark_tree(right, ignore);
        }
        Expr::Unary { operand, .. } => mark_tree(operand, ignore),
        Expr::Call { callee, args, .. } => {
            mark_tree(callee, ignore);
            for arg in args {
                mark_tree(arg, ignore);
            }
        }
        Expr::NamedArg { value, .. } | Expr::Member { object: value, .. } => {
            mark_tree(value, ignore)
        }
        Expr::Index { object, index } => {
            mark_tree(object, ignore);
            mark_tree(index, ignore);
        }
        Expr::Cast { value, .. } => mark_tree(value, ignore),
        Expr::NewArray { size, .. } => mark_tree(size, ignore),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Marks every expression that inherits a loose-mode `Wait`/`RegisterFor*`
/// argument exemption: arithmetic composition (`Binary`, `Unary`,
/// `NamedArg`) stays exempt, but a `Call`, `Member`, `Index`, `Cast`, or
/// `NewArray` boundary resets it.
fn mark_wait_exempt(expr: &Expr, ignore: &mut HashSet<*const Expr>) {
    ignore.insert(expr as *const Expr);
    match expr {
        Expr::Binary { left, right, .. } => {
            mark_wait_exempt(left, ignore);
            mark_wait_exempt(right, ignore);
        }
        Expr::Unary { operand, .. } => mark_wait_exempt(operand, ignore),
        Expr::NamedArg { value, .. } => mark_wait_exempt(value, ignore),
        _ => {}
    }
}

fn is_ignored_int(value: i64) -> bool {
    matches!(value, -1..=1)
}

fn is_ignored_float(value: f64) -> bool {
    value == -1.0 || value == 0.0 || value == 1.0
}

fn flag_int(value: i64, line: usize, store: &mut Store) {
    if !is_ignored_int(value) {
        push(value.to_string(), line, store);
    }
}

fn flag_float(value: f64, line: usize, store: &mut Store) {
    if !is_ignored_float(value) {
        push(value.to_string(), line, store);
    }
}

fn push(display: String, line: usize, store: &mut Store) {
    store.emit(
        line,
        1,
        format!(
            "[warning] Magic number {display}; extract it into a named constant, property, \
             or local variable so its meaning is clear"
        ),
        RULE,
    );
}

#[cfg(test)]
#[path = "magic_numbers_tests.rs"]
mod tests;

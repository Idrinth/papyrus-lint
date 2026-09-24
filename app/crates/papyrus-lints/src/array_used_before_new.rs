//! Flags an index or member/method access on a local array that's still
//! known to be `None` at that point (e.g. `Int[] a` followed by `a[0]` or
//! `a.Length`), since using an array that was never created with `New` is
//! the same runtime bug class as dereferencing a `None` Form: Papyrus logs
//! the mistake and silently no-ops the write or returns a default for a
//! read.
//!
//! This reuses [`crate::none_form_usage`]'s AST-based dataflow shape, but
//! tracks array-typed locals rather than object-typed Forms. A variable
//! becomes known-`None` when declared as `T[]` without an initializer, or
//! when assigned a literal `None`. It stops being tracked as soon as it's
//! assigned anything else (`new T[N]`, a call, another array), except that
//! assigning it another identifier makes it inherit that identifier's own
//! tracked state. Script-level array properties are *not* treated as
//! starting out `None`: the Creation Kit can fill them, so assuming they
//! need `New` would be noisy. Parameters are likewise left alone.

use std::collections::HashSet;

use papyrus_parser::ast::{
    AssignOp, BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Stmt, TypeName,
};

use crate::none_form_usage::{diverges, narrow_for_falsy, narrow_for_truthy};
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-used-before-new";

#[derive(Default)]
struct Collect {
    store: Store,
    none_vars: HashSet<String>,
    array_names: HashSet<String>,
    if_stack: Vec<IfFrame>,
    while_stack: Vec<WhileFrame>,
    expr_stack: Vec<ExprFrame>,
}

struct IfFrame {
    incoming: HashSet<String>,
    surviving: Vec<HashSet<String>>,
    branch_count: usize,
    seen_branches: usize,
    first_condition: Option<*const Expr>,
    pending_condition: Option<*const Expr>,
}

struct WhileFrame {
    incoming: HashSet<String>,
    condition: *const Expr,
}

enum ExprFrame {
    And {
        incoming: HashSet<String>,
        left: *const Expr,
    },
    Or {
        incoming: HashSet<String>,
        left: *const Expr,
    },
}

impl Collect {
    fn retain_arrays(&mut self) {
        let names = &self.array_names;
        self.none_vars.retain(|name| names.contains(name));
    }
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.none_vars.clear();
        self.array_names.clear();
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        match stmt {
            Stmt::If { branches, .. } => {
                self.if_stack.push(IfFrame {
                    incoming: self.none_vars.clone(),
                    surviving: Vec::new(),
                    branch_count: branches.len(),
                    seen_branches: 0,
                    first_condition: branches.first().map(|branch| ptr_of(&branch.condition)),
                    pending_condition: None,
                });
            }
            Stmt::While { condition, .. } => {
                self.while_stack.push(WhileFrame {
                    incoming: self.none_vars.clone(),
                    condition: ptr_of(condition),
                });
            }
            _ => {}
        }
    }

    fn visit_if_branch(&mut self, branch: &IfBranch, _ctx: &mut VisitCtx<'_>) {
        let Some(frame) = self.if_stack.last_mut() else {
            return;
        };
        self.none_vars.clone_from(&frame.incoming);
        frame.pending_condition = Some(ptr_of(&branch.condition));
    }

    fn leave_if_branch(&mut self, branch: &IfBranch, _ctx: &mut VisitCtx<'_>) {
        let Some(frame) = self.if_stack.last_mut() else {
            return;
        };
        if !diverges(&branch.body) {
            frame.surviving.push(self.none_vars.clone());
        }
        frame.seen_branches += 1;
        let finished = frame.seen_branches == frame.branch_count;
        let incoming = frame.incoming.clone();
        let first_condition = frame.first_condition.filter(|_| frame.branch_count == 1);
        if finished {
            self.none_vars = incoming;
            if let Some(condition) = first_condition {
                narrow_for_falsy(unsafe { &*condition }, &mut self.none_vars);
                self.retain_arrays();
            }
        }
    }

    fn leave_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        match stmt {
            Stmt::VarDecl(decl) => {
                if !is_array_type(&decl.type_name) {
                    return;
                }
                self.array_names.insert(decl.name.to_lowercase());
                if let Some(value) = &decl.value {
                    record_write(&decl.name, value, &mut self.none_vars);
                } else {
                    self.none_vars.insert(decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                ..
            } => {
                if let (Expr::Identifier(name), AssignOp::Assign) = (target, op) {
                    if self.array_names.contains(&name.to_lowercase()) {
                        record_write(name, value, &mut self.none_vars);
                    }
                }
            }
            Stmt::If { else_body, .. } => {
                let Some(frame) = self.if_stack.pop() else {
                    return;
                };
                let mut surviving = frame.surviving;
                if !diverges(else_body) {
                    surviving.push(self.none_vars.clone());
                }
                self.none_vars = if surviving.is_empty() {
                    frame.incoming
                } else {
                    surviving.into_iter().flatten().collect()
                };
                self.retain_arrays();
            }
            Stmt::While { condition, .. } => {
                let Some(frame) = self.while_stack.pop() else {
                    return;
                };
                self.none_vars = frame.incoming;
                narrow_for_falsy(condition, &mut self.none_vars);
                self.retain_arrays();
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr, _ctx: &mut VisitCtx<'_>) {
        match expr {
            Expr::Binary {
                left,
                op: BinaryOp::And,
                ..
            } => {
                self.expr_stack.push(ExprFrame::And {
                    incoming: self.none_vars.clone(),
                    left: ptr_of(left),
                });
            }
            Expr::Binary {
                left,
                op: BinaryOp::Or,
                ..
            } => {
                self.expr_stack.push(ExprFrame::Or {
                    incoming: self.none_vars.clone(),
                    left: ptr_of(left),
                });
            }
            _ => {}
        }
    }

    fn leave_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        match expr {
            Expr::Index { object, .. } => {
                flag_if_none(object, "indexing it", &self.none_vars, &mut self.store, ctx);
            }
            Expr::Member { object, property } => {
                flag_if_none(
                    object,
                    &format!("accessing '.{property}' on it"),
                    &self.none_vars,
                    &mut self.store,
                    ctx,
                );
            }
            _ => {}
        }

        let is_and_or_root = matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                ..
            }
        );
        if is_and_or_root {
            if let Some(frame) = self.expr_stack.pop() {
                self.none_vars = match frame {
                    ExprFrame::And { incoming, .. } | ExprFrame::Or { incoming, .. } => incoming,
                };
            }
        }

        match self.expr_stack.last() {
            Some(ExprFrame::And { left, .. }) if *left == ptr_of(expr) => {
                narrow_for_truthy(expr, &mut self.none_vars);
                self.retain_arrays();
            }
            Some(ExprFrame::Or { left, .. }) if *left == ptr_of(expr) => {
                narrow_for_falsy(expr, &mut self.none_vars);
                self.retain_arrays();
            }
            _ => {}
        }

        if self
            .if_stack
            .last()
            .and_then(|frame| frame.pending_condition)
            == Some(ptr_of(expr))
        {
            if let Some(frame) = self.if_stack.last_mut() {
                frame.pending_condition = None;
            }
            narrow_for_truthy(expr, &mut self.none_vars);
            self.retain_arrays();
        }

        if self.while_stack.last().map(|frame| frame.condition) == Some(ptr_of(expr)) {
            narrow_for_truthy(expr, &mut self.none_vars);
            self.retain_arrays();
        }
    }
}

fn ptr_of(expr: &Expr) -> *const Expr {
    expr as *const Expr
}

fn flag_if_none(
    object: &Expr,
    action: &str,
    none_vars: &HashSet<String>,
    store: &mut Store,
    ctx: &mut VisitCtx<'_>,
) {
    let Expr::Identifier(name) = object else {
        return;
    };
    if none_vars.contains(&name.to_lowercase()) {
        store.emit(
            ctx.line,
            1,
            format!(
                "[warning] '{name}' may still be None here; {action} will fail because the array was never created with New"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event in `source` for index or member access on a
/// local array that's still known to be `None`.
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

fn is_array_type(type_name: &TypeName) -> bool {
    type_name.is_array
}

/// Updates `none_vars` for a plain `name = value` write.
fn record_write(name: &str, value: &Expr, none_vars: &mut HashSet<String>) {
    let key = name.to_lowercase();
    let becomes_none = match value {
        Expr::Literal(Literal::None) => true,
        Expr::Identifier(source) => none_vars.contains(&source.to_lowercase()),
        _ => false,
    };
    if becomes_none {
        none_vars.insert(key);
    } else {
        none_vars.remove(&key);
    }
}

#[cfg(test)]
#[path = "array_used_before_new_tests.rs"]
mod tests;

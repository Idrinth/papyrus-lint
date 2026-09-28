//! Flags a member/method access whose receiver is itself a member, call, or
//! cast (e.g. `a.GetLinkTo(...).Kill()`), covering the gap left by
//! [`crate::none_form_usage`], which only flags when the receiver is a bare
//! identifier still known to be `None`.
//!
//! Intermediate call/property results are not proven `None`, so this rule is
//! opt-in and more speculative than `none-form-usage`. To keep noise down it
//! only considers chains rooted at a local, parameter, script-level
//! property/variable, `Self`, or `Parent` — not static script qualifiers such
//! as `Game.GetPlayer().AddItem(...)`. Array-index receivers are left to
//! [`crate::unchecked_array_element`].

use std::collections::HashSet;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "member-chain-none-usage";

#[derive(Default)]
struct Collect {
    store: Store,
    /// Script-level property and variable names (lowercased), inherited by
    /// every function as possible chain roots.
    script_names: HashSet<String>,
    /// Names in the current function that may root a tracked chain: script
    /// bindings plus this function's parameters and locals.
    scoped_names: HashSet<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.script_names.clear();
        for property in &script.properties {
            self.script_names.insert(property.name.to_lowercase());
        }
        for variable in &script.variables {
            self.script_names.insert(variable.name.to_lowercase());
        }
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.scoped_names = self.script_names.clone();
        for param in &function.params {
            self.scoped_names.insert(param.name.to_lowercase());
        }
    }

    fn leave_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        if let Stmt::VarDecl(decl) = stmt {
            self.scoped_names.insert(decl.name.to_lowercase());
        }
    }

    fn leave_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Member { object, property } = expr else {
            return;
        };
        if !is_chain_receiver(object) {
            return;
        }
        if !root_is_tracked(object, &self.scoped_names) {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[warning] accessing '.{property}' on a chained member/call result may crash if that result is None; assign it to a variable and check for None first"
            ),
            RULE,
        );
    }
}

/// Receivers `none-form-usage` skips (anything but a bare identifier) that
/// this sibling is willing to consider. `Index` is left to
/// `unchecked-array-element`.
fn is_chain_receiver(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { .. } | Expr::Member { .. } | Expr::Cast { .. }
    )
}

enum ChainRoot<'a> {
    Ident(&'a str),
    Self_,
    Parent,
}

fn chain_root(expr: &Expr) -> Option<ChainRoot<'_>> {
    match expr {
        Expr::Identifier(name) => Some(ChainRoot::Ident(name)),
        Expr::Self_ => Some(ChainRoot::Self_),
        Expr::Parent => Some(ChainRoot::Parent),
        Expr::Member { object, .. } => chain_root(object),
        Expr::Call { callee, .. } => chain_root(callee),
        Expr::Index { object, .. } => chain_root(object),
        Expr::Cast { value, .. } | Expr::Is { value, .. } => chain_root(value),
        Expr::Unary { operand, .. } => chain_root(operand),
        _ => None,
    }
}

fn root_is_tracked(expr: &Expr, scoped_names: &HashSet<String>) -> bool {
    match chain_root(expr) {
        Some(ChainRoot::Self_ | ChainRoot::Parent) => true,
        Some(ChainRoot::Ident(name)) => scoped_names.contains(&name.to_lowercase()),
        None => false,
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event in `source` for member/method access on a
/// chained member/call/cast receiver rooted at an in-script binding.
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

#[cfg(test)]
#[path = "member_chain_none_usage_tests.rs"]
mod tests;

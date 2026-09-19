//! Flags a member/method access on a local variable that's still known to
//! be `None` at that point (e.g. `Armor a = None` followed by
//! `a.GetName()`), since dereferencing a `None` Form crashes the script at
//! runtime.
//!
//! This works from the parsed AST, tracking which local variables are
//! definitely `None` as it walks each function body in order. A variable
//! becomes known-`None` when declared or assigned a literal `None`, or
//! when it's declared without an initializer at all and its type isn't one
//! of the primitive value types (`Int`/`Float`/`Bool`/`String`) — object-typed
//! locals (`Form` and its subtypes) default to `None` until assigned, unlike
//! primitives which get a non-`None` zero value. Script-level `Auto`/
//! `AutoReadOnly` properties get the same treatment by default: an
//! object-typed one with no explicit initializer (or an explicit `= None`)
//! isn't guaranteed to be filled in until something outside the script (the
//! CK's Property Manager, another script's `PropertyGet`/`PropertySet`,
//! `OnInit`, …) sets it, so each function starts out treating it as
//! possibly `None` too, same as an uninitialized local. Setting
//! [`crate::config::Config::assume_auto_properties_filled`] drops that
//! assumption for properties (not locals), since many projects consider it
//! noise once they trust their Property Manager setup. It stops being
//! tracked as
//! soon as it's assigned anything else, except that assigning it another
//! identifier makes it inherit that identifier's own tracked state instead
//! (so `a = b` keeps `a` known-`None` when `b` still is, rather than
//! clearing it). `If`/`Else`
//! branches are narrowed using the branch's own condition when it's a
//! direct `None` check (`x == None`, `x != None`, `!x`, or a bare `x`,
//! optionally combined with `&&`/`||`), and a branch that unconditionally
//! `Return`s doesn't contribute its exit state to what follows the `If` —
//! covering the common `If x == None \n Return \n EndIf` guard idiom. A
//! `While` loop can only exit when its condition is false (Papyrus has no
//! `break`/`continue`), so the condition is also used to narrow the state
//! after the loop. Anything less direct (a condition built from a call, a
//! member access, or more than one identifier) leaves the state
//! unchanged rather than guessing at it.

use std::collections::HashSet;

use papyrus_parser::ast::{
    AssignOp, BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName, UnaryOp,
};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "none-form-usage";

#[derive(Default)]
struct Collect {
    store: Store,
    none_vars: HashSet<String>,
    default_none_properties: HashSet<String>,
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

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        self.default_none_properties = if ctx.config.assume_auto_properties_filled {
            HashSet::new()
        } else {
            default_none_properties(script)
        };
    }

    fn visit_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.none_vars = self.default_none_properties.clone();
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
        if frame.seen_branches == frame.branch_count {
            self.none_vars.clone_from(&frame.incoming);
            if frame.branch_count == 1 {
                if let Some(condition) = frame.first_condition {
                    narrow_for_falsy(unsafe { &*condition }, &mut self.none_vars);
                }
            }
        }
    }

    fn leave_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    record_write(&decl.name, value, &mut self.none_vars);
                } else if is_object_type(&decl.type_name) {
                    self.none_vars.insert(decl.name.to_lowercase());
                } else {
                    self.none_vars.remove(&decl.name.to_lowercase());
                }
            }
            Stmt::Assign {
                target,
                op,
                value,
                ..
            } => {
                if let (Expr::Identifier(name), AssignOp::Assign) = (target, op) {
                    record_write(name, value, &mut self.none_vars);
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
            }
            Stmt::While { condition, .. } => {
                let Some(frame) = self.while_stack.pop() else {
                    return;
                };
                self.none_vars = frame.incoming;
                narrow_for_falsy(condition, &mut self.none_vars);
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
        if let Expr::Member { object, property } = expr {
            if let Expr::Identifier(name) = &**object {
                if self.none_vars.contains(&name.to_lowercase()) {
                    self.store.emit(
                        ctx.line,
                        1,
                        format!(
                            "[warning] '{name}' may still be None here; accessing '.{property}' on it will crash the script"
                        ),
                        RULE,
                    );
                }
            }
        }

        match self.expr_stack.last() {
            Some(ExprFrame::And { left, .. }) if *left == ptr_of(expr) => {
                narrow_for_truthy(expr, &mut self.none_vars);
            }
            Some(ExprFrame::Or { left, .. }) if *left == ptr_of(expr) => {
                narrow_for_falsy(expr, &mut self.none_vars);
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
        }

        if self.while_stack.last().map(|frame| frame.condition) == Some(ptr_of(expr)) {
            narrow_for_truthy(expr, &mut self.none_vars);
        }
    }
}

fn ptr_of(expr: &Expr) -> *const Expr {
    expr as *const Expr
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event in `source` for member/method access on a
/// local variable that's still known to be `None`. When
/// `assume_auto_properties_filled` is `true`, a script-level `Auto`/
/// `AutoReadOnly` property is never treated as possibly `None` on its own
/// (see [`crate::config::Config::assume_auto_properties_filled`]); a local
/// variable's own tracking is unaffected either way.
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

/// Script-level `Auto`/`AutoReadOnly` properties that default to `None`
/// until something outside the script (or a later statement) sets them:
/// object-typed ones with no explicit initializer, or an explicit `= None`.
/// Every function starts out treating these the same as an uninitialized
/// local of the same type.
fn default_none_properties(script: &Script) -> HashSet<String> {
    script
        .properties
        .iter()
        .filter(|property| property.is_auto || property.is_auto_read_only)
        .filter(|property| is_object_type(&property.type_name))
        .filter(|property| matches!(&property.value, None | Some(Expr::Literal(Literal::None))))
        .map(|property| property.name.to_lowercase())
        .collect()
}

/// Whether `type_name` is an object type (`Form` or one of its subtypes,
/// i.e. any script/native type other than the primitives) rather than one
/// of Papyrus's primitive value types. Object-typed locals default to
/// `None` when declared without an initializer; primitives get a non-`None`
/// zero value (`0`, `0.0`, `False`, `""`) instead.
pub(crate) fn is_object_type(type_name: &TypeName) -> bool {
    !type_name.is_array
        && !matches!(
            type_name.name.to_lowercase().as_str(),
            "int" | "float" | "bool" | "string"
        )
}

/// Updates `none_vars` for a plain `name = value` write (a declaration's
/// initializer or a `Stmt::Assign` with [`AssignOp::Assign`]): known-`None`
/// if `value` is the `None` literal, known-not-`None` otherwise — except
/// when `value` is itself an identifier, in which case `name` inherits
/// that identifier's current tracked state (aliasing a known-`None`
/// variable makes the target known-`None` too, rather than clearing it).
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

/// Whether `body` unconditionally exits its enclosing function, judged
/// (conservatively) by its last statement being a `Return`.
pub(crate) fn diverges(body: &[Stmt]) -> bool {
    matches!(body.last(), Some(Stmt::Return { .. }))
}

/// If `expr` is a direct `None` check on an identifier (`x == None`,
/// `None == x`, `x != None`, `!x`, or a bare `x`), returns its name
/// (lowercased) along with whether `expr` being *true* means that variable
/// is `None`.
fn none_check(expr: &Expr) -> Option<(String, bool)> {
    match expr {
        Expr::Identifier(name) => Some((name.to_lowercase(), false)),
        Expr::Unary {
            op: UnaryOp::Not,
            operand,
        } => none_check(operand).map(|(name, means_none)| (name, !means_none)),
        Expr::Binary {
            left,
            op: BinaryOp::Eq,
            right,
        } => none_literal_compare(left, right, true),
        Expr::Binary {
            left,
            op: BinaryOp::NotEq,
            right,
        } => none_literal_compare(left, right, false),
        _ => None,
    }
}

fn none_literal_compare(
    left: &Expr,
    right: &Expr,
    means_none_if_true: bool,
) -> Option<(String, bool)> {
    match (left, right) {
        (Expr::Identifier(name), Expr::Literal(Literal::None))
        | (Expr::Literal(Literal::None), Expr::Identifier(name)) => {
            Some((name.to_lowercase(), means_none_if_true))
        }
        _ => None,
    }
}

/// Narrows `state` to reflect `condition` having evaluated `true`,
/// recursing into `&&` operands (both must hold).
pub(crate) fn narrow_for_truthy(condition: &Expr, state: &mut HashSet<String>) {
    if let Some((name, means_none)) = none_check(condition) {
        if means_none {
            state.insert(name);
        } else {
            state.remove(&name);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::And,
        right,
    } = condition
    {
        narrow_for_truthy(left, state);
        narrow_for_truthy(right, state);
    }
}

/// Narrows `state` to reflect `condition` having evaluated `false`,
/// recursing into `||` operands (both must have been false).
pub(crate) fn narrow_for_falsy(condition: &Expr, state: &mut HashSet<String>) {
    if let Some((name, means_none)) = none_check(condition) {
        if means_none {
            state.remove(&name);
        } else {
            state.insert(name);
        }
        return;
    }
    if let Expr::Binary {
        left,
        op: BinaryOp::Or,
        right,
    } = condition
    {
        narrow_for_falsy(left, state);
        narrow_for_falsy(right, state);
    }
}

#[cfg(test)]
#[path = "none_form_usage_tests.rs"]
mod tests;

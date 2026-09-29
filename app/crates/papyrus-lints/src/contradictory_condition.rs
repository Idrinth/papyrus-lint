//! Flags an impossible pair of clauses under `&&` on the same identifier.
//!
//! Like [`crate::unreachable_elseif`] and [`crate::static_condition`], this
//! only looks at a narrow, provably safe shape: the same local or property
//! identifier compared against constant numeric thresholds. Calls, aliases,
//! `||` chains, and mixed identifiers are left unflagged rather than guessed
//! at.

use papyrus_parser::ast::{BinaryOp, Expr, IfBranch, Literal, Stmt, UnaryOp};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "contradictory-condition";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_if_branch(&mut self, branch: &IfBranch, _ctx: &mut VisitCtx<'_>) {
        check_condition(&branch.condition, branch.line, branch.col, &mut self.store);
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Stmt::While {
            condition,
            line,
            col,
            ..
        } = stmt
        else {
            return;
        };
        check_condition(condition, *line, *col, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every `If`/`ElseIf`/`While` condition in `source` for contradictory
/// clauses under `&&`.
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

fn check_condition(condition: &Expr, line: usize, column: usize, store: &mut Store) {
    walk_logic(condition, None, line, column, store);
}

/// Walks `expr`, analyzing each `&&` chain once at its root (when the parent
/// operator differs) so nested same-op trees are not reported twice.
fn walk_logic(
    expr: &Expr,
    parent_op: Option<BinaryOp>,
    line: usize,
    column: usize,
    store: &mut Store,
) {
    match expr {
        Expr::Binary { left, op, right } if matches!(op, BinaryOp::And | BinaryOp::Or) => {
            if *op == BinaryOp::And && parent_op != Some(BinaryOp::And) {
                analyze_and_chain(expr, line, column, store);
            }
            walk_logic(left, Some(*op), line, column, store);
            walk_logic(right, Some(*op), line, column, store);
        }
        Expr::Binary { left, right, .. } => {
            walk_logic(left, None, line, column, store);
            walk_logic(right, None, line, column, store);
        }
        Expr::Unary { operand, .. }
        | Expr::Member { object: operand, .. }
        | Expr::Cast { value: operand, .. }
        | Expr::Is { value: operand, .. } => {
            walk_logic(operand, None, line, column, store);
        }
        Expr::Index { object, index } => {
            walk_logic(object, None, line, column, store);
            walk_logic(index, None, line, column, store);
        }
        Expr::Call { callee, args, .. } => {
            walk_logic(callee, None, line, column, store);
            for arg in args {
                walk_logic(arg, None, line, column, store);
            }
        }
        Expr::NamedArg { value, .. } => walk_logic(value, None, line, column, store),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::NewArray { .. }
        | Expr::NewStruct { .. } => {}
    }
}

/// Collects the leaf clauses of an associative `&&` chain and flags pairwise
/// contradictions among comparable clauses on the same identifier.
fn analyze_and_chain(expr: &Expr, line: usize, column: usize, store: &mut Store) {
    let clauses = collect_and_clauses(expr);
    if clauses.len() < 2 {
        return;
    }

    let mut reported_pairs = 0usize;
    for i in 0..clauses.len() {
        for j in (i + 1)..clauses.len() {
            if clauses_contradict(clauses[i], clauses[j]) {
                store.emit(
                    line,
                    column,
                    "[warning] Condition clauses on the same identifier contradict \
                     each other and cannot both be true",
                    RULE,
                );
                reported_pairs += 1;
                // One diagnostic per conflicting pair is enough signal; cap
                // extreme chains so a long impossible list stays readable.
                if reported_pairs >= 3 {
                    return;
                }
            }
        }
    }
}

fn collect_and_clauses(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => {
            let mut clauses = collect_and_clauses(left);
            clauses.extend(collect_and_clauses(right));
            clauses
        }
        _ => vec![expr],
    }
}

fn clauses_contradict(left: &Expr, right: &Expr) -> bool {
    let Some((lhs_a, op_a, value_a)) = extract_comparison(left) else {
        return false;
    };
    let Some((lhs_b, op_b, value_b)) = extract_comparison(right) else {
        return false;
    };
    if !same_identifier(lhs_a, lhs_b) {
        return false;
    }
    let Some(interval_a) = Interval::from_comparison(op_a, value_a) else {
        return false;
    };
    let Some(interval_b) = Interval::from_comparison(op_b, value_b) else {
        return false;
    };
    interval_a.is_disjoint_from(&interval_b)
}

fn is_simple_ref(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) => true,
        Expr::Member { object, .. } => matches!(
            object.as_ref(),
            Expr::Identifier(_) | Expr::Self_ | Expr::Parent
        ),
        _ => false,
    }
}

fn same_identifier(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        (
            Expr::Member {
                object: lo,
                property: lp,
            },
            Expr::Member {
                object: ro,
                property: rp,
            },
        ) => lp.eq_ignore_ascii_case(rp) && same_identifier_object(lo, ro),
        _ => false,
    }
}

fn same_identifier_object(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Self_, Expr::Self_) | (Expr::Parent, Expr::Parent) => true,
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

fn extract_comparison(expr: &Expr) -> Option<(&Expr, BinaryOp, f64)> {
    let Expr::Binary { left, op, right } = expr else {
        return None;
    };
    if !matches!(
        op,
        BinaryOp::Eq | BinaryOp::Gt | BinaryOp::Lt | BinaryOp::GtEq | BinaryOp::LtEq
    ) {
        return None;
    }

    match (numeric_literal(left), numeric_literal(right)) {
        (None, Some(value)) if is_simple_ref(left) => Some((left, *op, value)),
        (Some(value), None) if is_simple_ref(right) => Some((right, flip(*op), value)),
        _ => None,
    }
}

fn numeric_literal(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value as f64),
        Expr::Literal(Literal::Float(value)) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => numeric_literal(operand).map(|value| -value),
        _ => None,
    }
}

fn flip(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::GtEq => BinaryOp::LtEq,
        BinaryOp::LtEq => BinaryOp::GtEq,
        other => other,
    }
}

#[derive(Clone, Copy)]
struct Bound {
    value: f64,
    inclusive: bool,
}

#[derive(Clone, Copy)]
struct Interval {
    low: Option<Bound>,
    high: Option<Bound>,
}

impl Interval {
    fn from_comparison(op: BinaryOp, value: f64) -> Option<Self> {
        match op {
            BinaryOp::Gt => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: false,
                }),
                high: None,
            }),
            BinaryOp::GtEq => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: true,
                }),
                high: None,
            }),
            BinaryOp::Lt => Some(Self {
                low: None,
                high: Some(Bound {
                    value,
                    inclusive: false,
                }),
            }),
            BinaryOp::LtEq => Some(Self {
                low: None,
                high: Some(Bound {
                    value,
                    inclusive: true,
                }),
            }),
            BinaryOp::Eq => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: true,
                }),
                high: Some(Bound {
                    value,
                    inclusive: true,
                }),
            }),
            _ => None,
        }
    }

    /// Whether this interval and `other` share no common value.
    fn is_disjoint_from(&self, other: &Self) -> bool {
        !bounds_allow(self.low, other.high) || !bounds_allow(other.low, self.high)
    }
}

/// Whether a lower bound and an upper bound still leave room for some value.
fn bounds_allow(low: Option<Bound>, high: Option<Bound>) -> bool {
    match (low, high) {
        (None, _) | (_, None) => true,
        (Some(l), Some(h)) => {
            if l.value < h.value {
                true
            } else if l.value > h.value {
                false
            } else {
                l.inclusive && h.inclusive
            }
        }
    }
}

#[cfg(test)]
#[path = "contradictory_condition_tests.rs"]
mod tests;

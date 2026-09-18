//! Flags an `ElseIf` branch whose condition can never be true, because an
//! earlier branch of the same `If` (the `If` itself or a prior `ElseIf`)
//! already covers every value that would satisfy it.
//!
//! Like [`crate::static_condition`], this only looks at a narrow, provably
//! safe shape rather than trying to prove reachability in general: both
//! branches' conditions must be a direct relational comparison
//! (`==`, `!=`, `<`, `<=`, `>`, `>=`) between the exact same left-hand
//! expression (matched structurally, e.g. the same identifier or member
//! access) and a numeric literal (optionally negated). A branch is flagged
//! the moment its own value range turns out to be a subset of an earlier
//! branch's, since that earlier branch is only skipped when its condition
//! is false — so a later branch that could only be true when the earlier
//! one is *also* true can itself never be reached. Anything else (compound
//! `&&`/`\|\|` conditions, a non-numeric operand, a differently-shaped
//! left-hand expression) is left unflagged rather than guessed at.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, UnaryOp};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unreachable-elseif";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// Checks every `If` statement in `source` for an `ElseIf` branch whose
/// condition is already fully covered by an earlier branch's condition.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(script) {
        check_body(&function.body, &mut diagnostics);
    }
    diagnostics
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                check_branches(branches, diagnostics);
                for branch in branches {
                    check_body(&branch.body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Checks every `ElseIf` branch (i.e. every branch but the first) against
/// each branch before it, flagging the first earlier branch found to
/// already cover it, if any.
fn check_branches(branches: &[IfBranch], diagnostics: &mut Vec<Diagnostic>) {
    for i in 1..branches.len() {
        let Some((lhs_i, op_i, value_i)) = extract_comparison(&branches[i].condition) else {
            continue;
        };
        let Some(interval_i) = Interval::from_comparison(op_i, value_i) else {
            continue;
        };

        for earlier in &branches[..i] {
            let Some((lhs_j, op_j, value_j)) = extract_comparison(&earlier.condition) else {
                continue;
            };
            if lhs_i != lhs_j {
                continue;
            }
            let Some(interval_j) = Interval::from_comparison(op_j, value_j) else {
                continue;
            };

            if interval_i.is_subset_of(&interval_j) {
                diagnostics.push(Diagnostic {
                    line: branches[i].line,
                    column: branches[i].col,
                    message: format!(
                        "[warning] This condition is already covered by the branch on line {}; it can never be true here",
                        earlier.line
                    ),
                    rule: RULE,
                });
                break;
            }
        }
    }
}

/// Splits a condition apart into its left-hand expression, its relational
/// operator, and its numeric right-hand value, normalizing a literal
/// written on the left (e.g. `9 < x`) back to the equivalent
/// expression-on-the-left form (`x > 9`) so both orderings compare the
/// same way. Returns `None` for anything else: a non-comparison, a
/// comparison with a literal (or no literal) on both sides, or a
/// non-numeric literal.
fn extract_comparison(expr: &Expr) -> Option<(&Expr, BinaryOp, f64)> {
    let Expr::Binary { left, op, right } = expr else {
        return None;
    };
    if !matches!(
        op,
        BinaryOp::Eq
            | BinaryOp::NotEq
            | BinaryOp::Gt
            | BinaryOp::Lt
            | BinaryOp::GtEq
            | BinaryOp::LtEq
    ) {
        return None;
    }

    match (numeric_literal(left), numeric_literal(right)) {
        (None, Some(value)) => Some((left, *op, value)),
        (Some(value), None) => Some((right, flip(*op), value)),
        _ => None,
    }
}

/// Folds `expr` down to a plain number when it's a numeric literal,
/// optionally negated (`-9`); anything else returns `None`.
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

/// The operator that keeps a comparison's meaning when its literal and
/// expression operands swap sides (e.g. `9 < x` and `x > 9` are the same
/// comparison).
fn flip(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::GtEq => BinaryOp::LtEq,
        BinaryOp::LtEq => BinaryOp::GtEq,
        other => other,
    }
}

/// One endpoint of an [`Interval`], `inclusive` telling whether the bound
/// value itself is part of the range.
#[derive(Clone, Copy)]
struct Bound {
    value: f64,
    inclusive: bool,
}

/// The set of numeric values that satisfy a single relational comparison
/// against a constant, as a low/high bound pair. `None` on either side
/// means unbounded in that direction.
#[derive(Clone, Copy)]
struct Interval {
    low: Option<Bound>,
    high: Option<Bound>,
}

impl Interval {
    /// The interval of values satisfying `<lhs> op value`, for the
    /// relational operators that describe a single contiguous range.
    /// `!=` excludes exactly one point rather than describing a
    /// contiguous range, so it's left unhandled (`None`) rather than
    /// approximated.
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

    /// Whether every value satisfying `self` also satisfies `other`.
    fn is_subset_of(&self, other: &Self) -> bool {
        low_rank(self.low) >= low_rank(other.low) && high_rank(self.high) <= high_rank(other.high)
    }
}

/// A comparable key for a lower bound: further right (greater) means more
/// restrictive. An unbounded lower bound sorts first; at the same value, an
/// exclusive bound (`>`) is more restrictive than an inclusive one (`>=`),
/// since it starts just past that value instead of at it.
fn low_rank(bound: Option<Bound>) -> (f64, i8) {
    match bound {
        None => (f64::NEG_INFINITY, 0),
        Some(Bound { value, inclusive }) => (value, if inclusive { 0 } else { 1 }),
    }
}

/// The upper-bound counterpart of [`low_rank`]: further left (smaller)
/// means more restrictive, an unbounded upper bound sorts last, and an
/// exclusive bound (`<`) is more restrictive than an inclusive one (`<=`)
/// at the same value.
fn high_rank(bound: Option<Bound>) -> (f64, i8) {
    match bound {
        None => (f64::INFINITY, 0),
        Some(Bound { value, inclusive }) => (value, if inclusive { 1 } else { 0 }),
    }
}

#[cfg(test)]
#[path = "unreachable_elseif_tests.rs"]
mod tests;

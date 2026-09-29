//! Flags an `If`/`Else` used only to assign `true`/`false` to the same
//! target. `If x > 5` / `bResult = true` / `Else` / `bResult = false` /
//! `EndIf` is the same as `bResult = x > 5`.
//!
//! Related but different: [`crate::boolean_simplification`] targets
//! `== true` / `== false` comparisons; [`crate::strict_boolean`] requires
//! Bool-typed conditions. This rule only looks at the If/Else shape.
//!
//! This works from the parsed AST rather than raw tokens, so a script that
//! doesn't parse cleanly is left unchecked rather than guessed at. Repair
//! still needs the token stream to locate `EndIf` and to recover the
//! condition / target spellings from source.

use papyrus_parser::ast::{AssignOp, Expr, Literal, Stmt};
use papyrus_parser::token::Token;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "redundant-bool-assignment";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        if classify(stmt).is_none() {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            "[info] If/Else only assigns true/false to the same target; use a direct \
             boolean expression instead",
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for an `If`/`Else` that only assigns `true`/`false` to
/// the same target. Flagged as an `[info]`.
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

/// Rewrites every If/Else [`check`] would flag into a single boolean
/// assignment. A line suppressed with `@disable` / `@disable-file` is left
/// unchanged. Hits are applied from the end of the file so earlier offsets
/// stay valid.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    let mut current = source.to_string();
    for _ in 0..16 {
        let Ok(parsed) = papyrus_parser::parse(&current) else {
            return current;
        };
        let Ok(tokens) = papyrus_parser::tokenize(&current) else {
            return current;
        };
        let disables = crate::disable_comments::Disables::scan(&current);
        let mut hits: Vec<Hit> = collect_hits(&parsed, &current, &tokens)
            .into_iter()
            .filter(|hit| !disables.is_disabled(hit.if_line, RULE))
            .collect();
        if hits.is_empty() {
            break;
        }
        hits.sort_by_key(|hit| std::cmp::Reverse(hit.start));
        let next = apply_edits(&current, &hits);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(super) struct Classified<'a> {
    inverted: bool,
    target: &'a Expr,
    condition: &'a Expr,
    if_line: usize,
}

pub(super) struct Hit {
    if_line: usize,
    start: usize,
    end: usize,
    replacement: String,
}

fn classify(stmt: &Stmt) -> Option<Classified<'_>> {
    let Stmt::If {
        branches,
        else_body,
        else_line,
        line,
        ..
    } = stmt
    else {
        return None;
    };
    if branches.len() != 1 || else_line.is_none() {
        return None;
    }
    let branch = &branches[0];
    let then_assign = single_bool_assign(&branch.body)?;
    let else_assign = single_bool_assign(else_body)?;
    if !targets_equal(then_assign.target, else_assign.target) {
        return None;
    }
    let then_bool = then_assign.value?;
    let else_bool = else_assign.value?;
    if then_bool == else_bool {
        return None;
    }
    Some(Classified {
        inverted: !then_bool,
        target: then_assign.target,
        condition: &branch.condition,
        if_line: *line,
    })
}

struct BoolAssign<'a> {
    target: &'a Expr,
    value: Option<bool>,
}

fn single_bool_assign(body: &[Stmt]) -> Option<BoolAssign<'_>> {
    if body.len() != 1 {
        return None;
    }
    let Stmt::Assign {
        target,
        op: AssignOp::Assign,
        value,
        ..
    } = &body[0]
    else {
        return None;
    };
    let Expr::Literal(Literal::Bool(lit)) = value else {
        return None;
    };
    Some(BoolAssign {
        target,
        value: Some(*lit),
    })
}

fn targets_equal(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        (Expr::Self_, Expr::Self_) => true,
        (
            Expr::Member {
                object: lo,
                property: lp,
            },
            Expr::Member {
                object: ro,
                property: rp,
            },
        ) => targets_equal(lo, ro) && lp.eq_ignore_ascii_case(rp),
        (
            Expr::Index {
                object: lo,
                index: li,
            },
            Expr::Index {
                object: ro,
                index: ri,
            },
        ) => targets_equal(lo, ro) && indexes_equal(li, ri),
        _ => false,
    }
}

fn indexes_equal(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (
            Expr::Literal(Literal::Int { value: a, .. }),
            Expr::Literal(Literal::Int { value: b, .. }),
        ) => a == b,
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

fn collect_hits(
    script: &papyrus_parser::ast::Script,
    source: &str,
    tokens: &[Token],
) -> Vec<Hit> {
    let mut hits = Vec::new();
    for function in script
        .functions
        .iter()
        .chain(script.states.iter().flat_map(|state| state.functions.iter()))
    {
        collect_stmt_hits(&function.body, source, tokens, &mut hits);
    }
    hits
}

fn collect_stmt_hits(body: &[Stmt], source: &str, tokens: &[Token], hits: &mut Vec<Hit>) {
    for stmt in body {
        if let Some(classified) = classify(stmt) {
            if let Some(hit) = build_hit(&classified, source, tokens) {
                hits.push(hit);
            }
        }
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_stmt_hits(&branch.body, source, tokens, hits);
                }
                collect_stmt_hits(else_body, source, tokens, hits);
            }
            Stmt::While { body, .. } => collect_stmt_hits(body, source, tokens, hits),
            Stmt::LockGuard { body, else_body, .. } => {
                collect_stmt_hits(body, source, tokens, hits);
                collect_stmt_hits(else_body, source, tokens, hits);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}


#[path = "redundant_bool_assignment_repair.rs"]
mod repair_helpers;
use repair_helpers::*;

#[cfg(test)]
#[path = "redundant_bool_assignment_tests.rs"]
mod tests;

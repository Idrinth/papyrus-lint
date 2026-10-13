//! Flags a `SetStage` call on the script's own quest that is not preceded,
//! within the same function or event, by a stage check (`GetStageDone`,
//! `GetStage`, `GetCurrentStageID`, `IsStageDone`).
//!
//! By default `SetStage` on an already-set stage is a no-op; only quests with
//! "Allow Repeated Stages" enabled re-run that stage's fragment, duplicating
//! its side effects. The linter cannot see quest flags, so the rule is opt-in
//! (`enabled_by_default: false`, not enabled by any preset). Checks living in
//! another function are not tracked, so the rule is informational and low
//! importance.
//!
//! Only scripts extending `Quest` (directly, or through a resolvable
//! parent) are checked, and only calls on the script itself (`SetStage(..)`
//! or `Self.SetStage(..)`). CreationKit fragment scripts (a `;BEGIN
//! FRAGMENT` marker or a `Fragment_<digits>` function, see
//! [`crate::fragment_code::is_fragment_script`]) are skipped entirely:
//! their code runs under stage, dialogue or scene conditions set elsewhere.

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use crate::fragment_code::is_fragment_script;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "quest-stage-logic-gap";

const STAGE_QUERIES: [&str; 4] = ["getstagedone", "getstage", "getcurrentstageid", "isstagedone"];

#[derive(Default)]
struct Collect {
    store: Store,
}

#[derive(PartialEq, Eq)]
enum CallKind {
    SetStage,
    Query,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        if is_fragment_script(ctx.source, Some(script)) || !extends_quest(script, ctx) {
            return;
        }
        let functions = script
            .functions
            .iter()
            .chain(script.states.iter().flat_map(|state| state.functions.iter()));
        for function in functions {
            self.check_function(function);
        }
    }
}

impl Collect {
    fn check_function(&mut self, function: &FunctionDecl) {
        let mut calls = Vec::new();
        for stmt in &function.body {
            stmt_calls(stmt, &mut calls);
        }
        calls.sort_by_key(|(line, col, _)| (*line, *col));
        let mut guarded = false;
        for (line, col, kind) in calls {
            match kind {
                CallKind::Query => guarded = true,
                CallKind::SetStage if !guarded => self.store.emit(
                    line,
                    col,
                    "[info] SetStage is not preceded by a GetStageDone/GetStage check in this \
                     function; only relevant if the quest has Allow Repeated Stages enabled, \
                     where setting an already-set stage re-runs its fragment"
                        .to_string(),
                    RULE,
                ),
                CallKind::SetStage => {}
            }
        }
    }
}

fn extends_quest(script: &Script, ctx: &mut VisitCtx<'_>) -> bool {
    script.extends.as_deref().is_some_and(|parent| {
        parent.eq_ignore_ascii_case("Quest") || ctx.external.is_subtype(parent, "Quest")
    })
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for unguarded `SetStage` calls in quest scripts.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn stmt_calls(stmt: &Stmt, out: &mut Vec<(usize, usize, CallKind)>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                expr_calls(value, out);
            }
        }
        Stmt::Assign { target, value, .. } => {
            expr_calls(target, out);
            expr_calls(value, out);
        }
        Stmt::Expr { value, .. } => expr_calls(value, out),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                expr_calls(value, out);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                expr_calls(&branch.condition, out);
                branch.body.iter().for_each(|s| stmt_calls(s, out));
            }
            else_body.iter().for_each(|s| stmt_calls(s, out));
        }
        Stmt::While {
            condition, body, ..
        } => {
            expr_calls(condition, out);
            body.iter().for_each(|s| stmt_calls(s, out));
        }
        Stmt::LockGuard {
            body, else_body, ..
        } => {
            body.iter().for_each(|s| stmt_calls(s, out));
            else_body.iter().for_each(|s| stmt_calls(s, out));
        }
    }
}

fn expr_calls(expr: &Expr, out: &mut Vec<(usize, usize, CallKind)>) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            // Arguments are evaluated before the call itself.
            args.iter().for_each(|arg| expr_calls(arg, out));
            let (receiver, name) = match callee.as_ref() {
                Expr::Identifier(name) => (None, name.as_str()),
                Expr::Member { object, property } => (Some(object.as_ref()), property.as_str()),
                other => {
                    expr_calls(other, out);
                    return;
                }
            };
            if let Some(receiver) = receiver {
                expr_calls(receiver, out);
            }
            let lower = name.to_ascii_lowercase();
            if STAGE_QUERIES.contains(&lower.as_str()) {
                out.push((*line, *col, CallKind::Query));
            } else if lower == "setstage"
                && receiver.is_none_or(|receiver| matches!(receiver, Expr::Self_))
            {
                out.push((*line, *col, CallKind::SetStage));
            }
        }
        Expr::Binary { left, right, .. } => {
            expr_calls(left, out);
            expr_calls(right, out);
        }
        Expr::Unary { operand, .. } => expr_calls(operand, out),
        Expr::NamedArg { value, .. } | Expr::Cast { value, .. } | Expr::Is { value, .. } => {
            expr_calls(value, out)
        }
        Expr::Member { object, .. } => expr_calls(object, out),
        Expr::Index { object, index } => {
            expr_calls(object, out);
            expr_calls(index, out);
        }
        Expr::NewArray { size, .. } => expr_calls(size, out),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::NewStruct { .. } => {}
    }
}

#[cfg(test)]
#[path = "quest_stage_logic_gap_tests.rs"]
mod tests;

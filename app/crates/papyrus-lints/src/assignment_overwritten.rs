//! Flags a write to a local variable that is overwritten by a later
//! assignment with no read of that value in between.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to tell a local's declaration and write sites apart from an actual
//! read of it; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. Only function/event locals are tracked —
//! parameters and script properties are out of scope. Papyrus has no
//! block scoping, so a local declared inside an `If`/`While` body is
//! still matched by name (case-insensitively) for the rest of its
//! enclosing function.

use std::collections::{HashMap, HashSet};

use papyrus_parser::ast::{AssignOp, Expr, FunctionDecl, IfBranch, LockKind, Stmt, VariableDecl};

use crate::none_form_usage::diverges;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "assignment-overwritten";

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

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        let mut diagnostics = Vec::new();
        check_function(function, &self.protected, &mut diagnostics);
        self.store.extend(diagnostics);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for a local-variable write overwritten before it is
/// read. Flagged as a `[warning]`.
///
/// A write inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers,
/// is never flagged: it's generated boilerplate the user can't edit.
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

#[derive(Debug, Clone)]
struct PendingWrite {
    line: usize,
    name: String,
}

type Pending = HashMap<String, PendingWrite>;

struct Analysis<'a> {
    locals: &'a HashSet<String>,
    pending: &'a mut Pending,
    /// Incoming writes from the current control-flow split. Overwriting
    /// one of these on a single branch is not enough to flag it; see
    /// [`apply_split_exit`].
    suppressed: &'a Pending,
    suppressed_reads: &'a mut HashSet<String>,
    protected: &'a [bool],
    emitted: &'a mut HashSet<usize>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

fn check_function(function: &FunctionDecl, protected: &[bool], diagnostics: &mut Vec<Diagnostic>) {
    let locals = collect_local_names(&function.body);
    if locals.is_empty() {
        return;
    }

    let mut pending = Pending::new();
    let suppressed = Pending::new();
    let mut suppressed_reads = HashSet::new();
    let mut emitted = HashSet::new();
    let mut analysis = Analysis {
        locals: &locals,
        pending: &mut pending,
        suppressed: &suppressed,
        suppressed_reads: &mut suppressed_reads,
        protected,
        emitted: &mut emitted,
        diagnostics,
    };
    walk_body(&function.body, &mut analysis);
}

fn collect_local_names(body: &[Stmt]) -> HashSet<String> {
    collect_var_decls(body)
        .into_iter()
        .map(|decl| decl.name.to_lowercase())
        .collect()
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
            Stmt::LockGuard { body, else_body, .. } => {
                decls.extend(collect_var_decls(body));
                decls.extend(collect_var_decls(else_body));
            }
            _ => {}
        }
    }
    decls
}

fn walk_body(body: &[Stmt], analysis: &mut Analysis<'_>) {
    for stmt in body {
        walk_stmt(stmt, analysis);
    }
}

fn walk_stmt(stmt: &Stmt, analysis: &mut Analysis<'_>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr_as_read(value, analysis);
            }
            if decl.value.is_some() {
                record_write(&decl.name, decl.line, analysis);
            }
        }
        Stmt::Assign {
            target,
            op,
            value,
            line,
        } => {
            walk_expr_as_read(value, analysis);
            match (target, op) {
                (Expr::Identifier(name), AssignOp::Assign) => {
                    record_write(name, *line, analysis);
                }
                (Expr::Identifier(name), _) => {
                    // A compound assignment reads the current value
                    // before writing the new one.
                    note_read(name, analysis);
                    record_write(name, *line, analysis);
                }
                _ => walk_expr_as_read(target, analysis),
            }
        }
        Stmt::Expr { value, .. } => walk_expr_as_read(value, analysis),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr_as_read(value, analysis);
            }
        }
        Stmt::If {
            branches,
            else_body,
            else_line,
            ..
        } => handle_split(branches, else_body, else_line.is_some(), analysis),
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr_as_read(condition, analysis);
            // A `While` may run zero times, so the body cannot overwrite
            // a write from before the loop. Dead stores wholly inside the
            // body are still flagged by walking a clone of the incoming
            // pending writes.
            let mut loop_pending = analysis.pending.clone();
            let loop_suppressed = analysis.pending.clone();
            let mut loop_reads = HashSet::new();
            let mut loop_analysis = Analysis {
                locals: analysis.locals,
                pending: &mut loop_pending,
                suppressed: &loop_suppressed,
                suppressed_reads: &mut loop_reads,
                protected: analysis.protected,
                emitted: analysis.emitted,
                diagnostics: analysis.diagnostics,
            };
            walk_body(body, &mut loop_analysis);
        }
        Stmt::LockGuard {
            kind,
            body,
            else_body,
            else_line,
            ..
        } => match kind {
            LockKind::Lock => walk_body(body, analysis),
            LockKind::Try => handle_try_lock(body, else_body, else_line.is_some(), analysis),
        },
    }
}

fn handle_split(
    branches: &[IfBranch],
    else_body: &[Stmt],
    has_else: bool,
    analysis: &mut Analysis<'_>,
) {
    let entry = analysis.pending.clone();
    let mut exits = Vec::new();
    let mut read_keys = HashSet::new();

    for branch in branches {
        let mut state = entry.clone();
        let mut branch_reads = HashSet::new();
        let mut branch_analysis = Analysis {
            locals: analysis.locals,
            pending: &mut state,
            suppressed: &entry,
            suppressed_reads: &mut branch_reads,
            protected: analysis.protected,
            emitted: analysis.emitted,
            diagnostics: analysis.diagnostics,
        };
        walk_expr_as_read(&branch.condition, &mut branch_analysis);
        walk_body(&branch.body, &mut branch_analysis);
        read_keys.extend(branch_reads);
        if !diverges(&branch.body) {
            exits.push(state);
        }
    }

    if has_else {
        let mut state = entry.clone();
        let mut else_reads = HashSet::new();
        let mut else_analysis = Analysis {
            locals: analysis.locals,
            pending: &mut state,
            suppressed: &entry,
            suppressed_reads: &mut else_reads,
            protected: analysis.protected,
            emitted: analysis.emitted,
            diagnostics: analysis.diagnostics,
        };
        walk_body(else_body, &mut else_analysis);
        read_keys.extend(else_reads);
        if !diverges(else_body) {
            exits.push(state);
        }
    } else {
        exits.push(entry.clone());
    }

    apply_split_exit(&entry, &exits, &read_keys, analysis);
}

fn handle_try_lock(
    body: &[Stmt],
    else_body: &[Stmt],
    has_else: bool,
    analysis: &mut Analysis<'_>,
) {
    let entry = analysis.pending.clone();
    let mut taken = entry.clone();
    let mut taken_reads = HashSet::new();
    let mut taken_analysis = Analysis {
        locals: analysis.locals,
        pending: &mut taken,
        suppressed: &entry,
        suppressed_reads: &mut taken_reads,
        protected: analysis.protected,
        emitted: analysis.emitted,
        diagnostics: analysis.diagnostics,
    };
    walk_body(body, &mut taken_analysis);

    let mut exits = Vec::new();
    let mut read_keys = taken_reads;
    if !diverges(body) {
        exits.push(taken);
    }
    if has_else {
        let mut alternate = entry.clone();
        let mut else_reads = HashSet::new();
        let mut else_analysis = Analysis {
            locals: analysis.locals,
            pending: &mut alternate,
            suppressed: &entry,
            suppressed_reads: &mut else_reads,
            protected: analysis.protected,
            emitted: analysis.emitted,
            diagnostics: analysis.diagnostics,
        };
        walk_body(else_body, &mut else_analysis);
        read_keys.extend(else_reads);
        if !diverges(else_body) {
            exits.push(alternate);
        }
    } else {
        exits.push(entry.clone());
    }
    apply_split_exit(&entry, &exits, &read_keys, analysis);
}

/// After a split, keep an incoming write when some surviving path still
/// has it unread. If every surviving path dropped it, flag it only when
/// no path read that incoming value — otherwise a later assignment on
/// one branch would look like it killed a write another branch used.
fn apply_split_exit(
    entry: &Pending,
    exits: &[Pending],
    read_keys: &HashSet<String>,
    analysis: &mut Analysis<'_>,
) {
    if exits.is_empty() {
        analysis.pending.clear();
        return;
    }

    let mut next = Pending::new();
    for (key, write) in entry {
        let still_pending = exits.iter().any(|exit| {
            exit.get(key)
                .is_some_and(|current| current.line == write.line)
        });
        if still_pending {
            next.insert(key.clone(), write.clone());
            continue;
        }
        if !read_keys.contains(key) {
            emit_overwrite(write, analysis);
        }
    }
    *analysis.pending = next;
}

fn record_write(name: &str, line: usize, analysis: &mut Analysis<'_>) {
    let key = name.to_lowercase();
    if !analysis.locals.contains(&key) {
        return;
    }
    let next = PendingWrite {
        line,
        name: name.to_string(),
    };
    if let Some(previous) = analysis.pending.insert(key.clone(), next) {
        let suppressed = analysis
            .suppressed
            .get(&key)
            .is_some_and(|write| write.line == previous.line);
        if !suppressed {
            emit_overwrite(&previous, analysis);
        }
    }
}

fn emit_overwrite(write: &PendingWrite, analysis: &mut Analysis<'_>) {
    if analysis
        .protected
        .get(write.line)
        .copied()
        .unwrap_or(false)
    {
        return;
    }
    if !analysis.emitted.insert(write.line) {
        return;
    }
    analysis.diagnostics.push(Diagnostic {
        line: write.line,
        column: 1,
        message: format!(
            "[warning] Assignment to local variable '{}' is overwritten before its value is read",
            write.name
        ),
        rule: RULE,
    });
}

fn note_read(name: &str, analysis: &mut Analysis<'_>) {
    let key = name.to_lowercase();
    if analysis.pending.get(&key).is_some_and(|current| {
        analysis
            .suppressed
            .get(&key)
            .is_some_and(|incoming| incoming.line == current.line)
    }) {
        analysis.suppressed_reads.insert(key.clone());
    }
    analysis.pending.remove(&key);
}

fn walk_expr_as_read(expr: &Expr, analysis: &mut Analysis<'_>) {
    match expr {
        Expr::Identifier(name) => note_read(name, analysis),
        Expr::Binary { left, right, .. } => {
            walk_expr_as_read(left, analysis);
            walk_expr_as_read(right, analysis);
        }
        Expr::Unary { operand, .. } => walk_expr_as_read(operand, analysis),
        Expr::Call { callee, args, .. } => {
            walk_expr_as_read(callee, analysis);
            for arg in args {
                walk_expr_as_read(arg, analysis);
            }
        }
        Expr::Member { object, .. } => walk_expr_as_read(object, analysis),
        Expr::Index { object, index } => {
            walk_expr_as_read(object, analysis);
            walk_expr_as_read(index, analysis);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => walk_expr_as_read(value, analysis),
        Expr::NewArray { size, .. } => walk_expr_as_read(size, analysis),
        Expr::NamedArg { value, .. } => walk_expr_as_read(value, analysis),
        Expr::Literal(_) | Expr::Self_ | Expr::Parent | Expr::NewStruct { .. } => {}
    }
}

#[cfg(test)]
#[path = "assignment_overwritten_tests.rs"]
mod tests;

//! Flags a call to a side-effecting function nested inside any `Debug.*`
//! argument list.
//!
//! `Debug.Trace` / `Debug.Notification` / other `Debug.*` calls are often
//! compiled out, gated behind a debug flag, or skipped in release-like
//! playthroughs. An argument that itself calls something like
//! `RemoveItem` or a same-script function that writes a property then
//! only runs when that debug call runs, which is almost never what the
//! author intended.
//!
//! Matching is token-based so a script that doesn't parse cleanly is
//! still checked. Same-script side effects (a function that writes a
//! property/field, or transitively calls one that does) are proven from
//! the AST when the script parses; calls that can't be proven that way
//! are classified by a conservative name heuristic covering common
//! mutating native prefixes (`Set`, `Remove`, `Wait`, …).

use std::collections::{HashMap, HashSet};

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};
use papyrus_parser::token::{Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "debug-side-effects";

/// Native / conventional names that mutate game or script state even
/// when this script's own function list can't see their bodies.
const SIDE_EFFECT_PREFIXES: &[&str] = &[
    "set",
    "mod",
    "remove",
    "add",
    "enable",
    "disable",
    "force",
    "drop",
    "equip",
    "unequip",
    "delete",
    "reset",
    "start",
    "stop",
    "play",
    "interrupt",
    "evaluate",
    "move",
    "push",
    "apply",
    "cast",
    "dispel",
    "unlock",
    "lock",
    "kill",
    "resurrect",
    "place",
    "damage",
    "restore",
    "clear",
    "wait",
    "send",
    "register",
    "unregister",
    "goto",
    "fire",
];

/// Checks `source` for a side-effecting call nested inside any `Debug.*`
/// argument list. Flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    let same_script = same_script_side_effects(source);
    let mut diagnostics = Vec::new();
    let mut i = 0;
    while i + 3 < tokens.len() {
        if is_debug_call(&tokens, i) {
            let method = match &tokens[i + 2].kind {
                TokenKind::Identifier(name) => name.clone(),
                _ => {
                    i += 1;
                    continue;
                }
            };
            let open = i + 3;
            if let Some(close) = matching_rparen(&tokens, open) {
                collect_nested_calls(
                    &tokens,
                    open + 1,
                    close,
                    &method,
                    &same_script,
                    &mut diagnostics,
                );
                i = close + 1;
                continue;
            }
        }
        i += 1;
    }
    diagnostics
}

fn is_debug_call(tokens: &[Token], i: usize) -> bool {
    matches!(
        (
            &tokens[i].kind,
            &tokens[i + 1].kind,
            &tokens[i + 2].kind,
            &tokens[i + 3].kind,
        ),
        (
            TokenKind::Identifier(qualifier),
            TokenKind::Dot,
            TokenKind::Identifier(_),
            TokenKind::LParen,
        ) if qualifier.eq_ignore_ascii_case("Debug")
    )
}

fn matching_rparen(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_nested_calls(
    tokens: &[Token],
    from: usize,
    to: usize,
    debug_method: &str,
    same_script: &HashMap<String, bool>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut i = from;
    while i + 1 < to {
        if let TokenKind::Identifier(name) = &tokens[i].kind {
            if matches!(tokens[i + 1].kind, TokenKind::LParen)
                && looks_side_effecting(name, same_script)
            {
                diagnostics.push(Diagnostic {
                    line: tokens[i].line,
                    column: tokens[i].col,
                    message: format!(
                        "[warning] Call to '{name}' inside Debug.{debug_method} can run side \
                         effects only when that debug call executes; move the call out of the \
                         Debug argument list"
                    ),
                    rule: RULE,
                });
            }
        }
        i += 1;
    }
}

fn looks_side_effecting(name: &str, same_script: &HashMap<String, bool>) -> bool {
    let key = name.to_ascii_lowercase();
    if let Some(&proven) = same_script.get(&key) {
        return proven;
    }
    SIDE_EFFECT_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

fn same_script_side_effects(source: &str) -> HashMap<String, bool> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return HashMap::new();
    };
    side_effects_by_name(&collect_decls(&script))
}

fn collect_decls(script: &Script) -> HashMap<String, &FunctionDecl> {
    let mut decls = HashMap::new();
    for function in &script.functions {
        decls
            .entry(function.name.to_ascii_lowercase())
            .or_insert(function);
    }
    for state in &script.states {
        for function in &state.functions {
            decls
                .entry(function.name.to_ascii_lowercase())
                .or_insert(function);
        }
    }
    decls
}

fn side_effects_by_name(decls: &HashMap<String, &FunctionDecl>) -> HashMap<String, bool> {
    let mut result: HashMap<String, bool> = HashMap::with_capacity(decls.len());
    let mut calls: HashMap<String, HashSet<String>> = HashMap::with_capacity(decls.len());

    for (name, decl) in decls {
        let locals = local_names(decl);
        let mut writes = false;
        let mut called = HashSet::new();
        scan_stmts(&decl.body, &locals, &mut writes, &mut called);
        result.insert(name.clone(), writes);
        calls.insert(name.clone(), called);
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (name, callees) in &calls {
            if result[name] {
                continue;
            }
            if callees
                .iter()
                .any(|callee| result.get(callee).copied().unwrap_or(false))
            {
                result.insert(name.clone(), true);
                changed = true;
            }
        }
    }
    result
}

fn local_names(decl: &FunctionDecl) -> HashSet<String> {
    let mut locals: HashSet<String> = decl
        .params
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    collect_var_decl_names(&decl.body, &mut locals);
    locals
}

fn collect_var_decl_names(body: &[Stmt], locals: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                locals.insert(decl.name.to_ascii_lowercase());
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_var_decl_names(&branch.body, locals);
                }
                collect_var_decl_names(else_body, locals);
            }
            Stmt::While { body, .. } => collect_var_decl_names(body, locals),
            _ => {}
        }
    }
}

fn scan_stmts(
    body: &[Stmt],
    locals: &HashSet<String>,
    writes: &mut bool,
    called: &mut HashSet<String>,
) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    scan_expr(value, called);
                }
            }
            Stmt::Assign { target, value, .. } => {
                if writes_field(target, locals) {
                    *writes = true;
                }
                scan_expr(target, called);
                scan_expr(value, called);
            }
            Stmt::Expr { value, .. } => scan_expr(value, called),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    scan_expr(value, called);
                }
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    scan_expr(&branch.condition, called);
                    scan_stmts(&branch.body, locals, writes, called);
                }
                scan_stmts(else_body, locals, writes, called);
            }
            Stmt::While {
                condition, body, ..
            } => {
                scan_expr(condition, called);
                scan_stmts(body, locals, writes, called);
            }
        }
    }
}

fn writes_field(target: &Expr, locals: &HashSet<String>) -> bool {
    match target {
        Expr::Identifier(name) => !locals.contains(&name.to_ascii_lowercase()),
        Expr::Member { .. } => true,
        _ => false,
    }
}

fn scan_expr(expr: &Expr, called: &mut HashSet<String>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            scan_expr(left, called);
            scan_expr(right, called);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, called),
        Expr::Call { callee, args, .. } => {
            if let Some(name) = called_same_script_name(callee) {
                called.insert(name);
            }
            scan_expr(callee, called);
            for arg in args {
                scan_expr(arg, called);
            }
        }
        Expr::NamedArg { value, .. } => scan_expr(value, called),
        Expr::Member { object, .. } => scan_expr(object, called),
        Expr::Index { object, index } => {
            scan_expr(object, called);
            scan_expr(index, called);
        }
        Expr::Cast { value, .. } => scan_expr(value, called),
        Expr::NewArray { size, .. } => scan_expr(size, called),
    }
}

fn called_same_script_name(callee: &Expr) -> Option<String> {
    match callee {
        Expr::Identifier(name) => Some(name.to_ascii_lowercase()),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(property.to_ascii_lowercase())
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "debug_side_effects_tests.rs"]
mod tests;

//! Shared read/write classification for script properties.
//!
//! A name on another object (`other.Score`) is not a use of this script's
//! `Score`. `self.Score` and a bare `Score` identifier are.

use papyrus_parser::ast::{Expr, FunctionDecl, PropertyDecl, Script, Stmt};
use papyrus_parser::comment_annotations::parse_line_annotations;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, Copy)]
pub struct Usage {
    pub read: bool,
    pub written: bool,
}

pub fn line_has_external(source: &str, line: usize) -> bool {
    source
        .lines()
        .nth(line.saturating_sub(1))
        .is_some_and(|text| {
            parse_line_annotations(text)
                .iter()
                .any(|annotation| annotation.name.eq_ignore_ascii_case("external"))
        })
}

pub fn all_properties(script: &Script) -> Vec<&PropertyDecl> {
    let mut properties: Vec<&PropertyDecl> = script.properties.iter().collect();
    for group in &script.groups {
        properties.extend(group.properties.iter());
    }
    properties
}

pub fn collect_usage(script: &Script) -> HashMap<String, Usage> {
    let mut usage = HashMap::new();
    for property in all_properties(script) {
        usage.entry(property.name.to_ascii_lowercase()).or_default();
    }
    let names: HashSet<String> = usage.keys().cloned().collect();
    if names.is_empty() {
        return usage;
    }

    for function in &script.functions {
        walk_function(function, &names, &mut usage);
    }
    for state in &script.states {
        for function in &state.functions {
            walk_function(function, &names, &mut usage);
        }
    }
    for property in all_properties(script) {
        for accessor in &property.accessors {
            walk_function(accessor, &names, &mut usage);
        }
    }
    usage
}

pub fn backing_fields(property: &PropertyDecl) -> Vec<String> {
    let mut fields = Vec::new();
    for accessor in &property.accessors {
        let params: HashSet<String> = accessor
            .params
            .iter()
            .map(|param| param.name.to_ascii_lowercase())
            .collect();
        collect_backing_from_body(&accessor.body, &params, &mut fields);
    }
    fields
}

pub fn written_outside_accessors(
    script: &Script,
    property: &PropertyDecl,
    backing: &[String],
) -> bool {
    let mut names: HashSet<String> = HashSet::new();
    names.insert(property.name.to_ascii_lowercase());
    for field in backing {
        names.insert(field.clone());
    }

    for variable in &script.variables {
        if names.contains(&variable.name.to_ascii_lowercase()) && variable.value.is_some() {
            return true;
        }
    }

    let mut usage = HashMap::new();
    for name in &names {
        usage.insert(name.clone(), Usage::default());
    }
    for function in &script.functions {
        walk_function(function, &names, &mut usage);
    }
    for state in &script.states {
        for function in &state.functions {
            walk_function(function, &names, &mut usage);
        }
    }
    usage.values().any(|entry| entry.written)
}

fn walk_function(
    function: &FunctionDecl,
    names: &HashSet<String>,
    usage: &mut HashMap<String, Usage>,
) {
    walk_body(&function.body, names, usage);
}

fn walk_body(body: &[Stmt], names: &HashSet<String>, usage: &mut HashMap<String, Usage>) {
    for stmt in body {
        walk_stmt(stmt, names, usage);
    }
}

fn walk_stmt(stmt: &Stmt, names: &HashSet<String>, usage: &mut HashMap<String, Usage>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr_as_read(value, names, usage);
            }
        }
        Stmt::Assign {
            target, op, value, ..
        } => {
            walk_expr_as_read(value, names, usage);
            if let Some(name) = property_target_name(target, names) {
                if let Some(entry) = usage.get_mut(&name) {
                    entry.written = true;
                    let _ = op;
                }
            } else {
                walk_expr_as_read(target, names, usage);
            }
        }
        Stmt::Expr { value, .. } => walk_expr_as_read(value, names, usage),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr_as_read(value, names, usage);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                walk_expr_as_read(&branch.condition, names, usage);
                walk_body(&branch.body, names, usage);
            }
            walk_body(else_body, names, usage);
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr_as_read(condition, names, usage);
            walk_body(body, names, usage);
        }
        Stmt::LockGuard {
            body, else_body, ..
        } => {
            walk_body(body, names, usage);
            walk_body(else_body, names, usage);
        }
    }
}

fn property_target_name(target: &Expr, names: &HashSet<String>) -> Option<String> {
    match target {
        Expr::Identifier(name) => {
            let lower = name.to_ascii_lowercase();
            names.contains(&lower).then_some(lower)
        }
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            let lower = property.to_ascii_lowercase();
            names.contains(&lower).then_some(lower)
        }
        _ => None,
    }
}

fn walk_expr_as_read(expr: &Expr, names: &HashSet<String>, usage: &mut HashMap<String, Usage>) {
    match expr {
        Expr::Identifier(name) => mark_read(name, names, usage),
        Expr::Binary { left, right, .. } => {
            walk_expr_as_read(left, names, usage);
            walk_expr_as_read(right, names, usage);
        }
        Expr::Unary { operand, .. } => walk_expr_as_read(operand, names, usage),
        Expr::Call { callee, args, .. } => {
            walk_expr_as_read(callee, names, usage);
            for arg in args {
                walk_expr_as_read(arg, names, usage);
            }
        }
        Expr::Member { object, property } => {
            if matches!(object.as_ref(), Expr::Self_) {
                mark_read(property, names, usage);
            } else {
                walk_expr_as_read(object, names, usage);
            }
        }
        Expr::Index { object, index } => {
            walk_expr_as_read(object, names, usage);
            walk_expr_as_read(index, names, usage);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => walk_expr_as_read(value, names, usage),
        Expr::NewArray { size, .. } => walk_expr_as_read(size, names, usage),
        Expr::NamedArg { value, .. } => walk_expr_as_read(value, names, usage),
        Expr::Literal(_) | Expr::Self_ | Expr::Parent | Expr::NewStruct { .. } => {}
    }
}

fn mark_read(name: &str, names: &HashSet<String>, usage: &mut HashMap<String, Usage>) {
    let lower = name.to_ascii_lowercase();
    if names.contains(&lower) {
        if let Some(entry) = usage.get_mut(&lower) {
            entry.read = true;
        }
    }
}

fn collect_backing_from_body(body: &[Stmt], params: &HashSet<String>, fields: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Stmt::Return {
                value: Some(Expr::Identifier(name)),
                ..
            } => push_field(name, params, fields),
            Stmt::Assign {
                target: Expr::Identifier(name),
                ..
            } => push_field(name, params, fields),
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_backing_from_body(&branch.body, params, fields);
                }
                collect_backing_from_body(else_body, params, fields);
            }
            Stmt::While { body, .. } => collect_backing_from_body(body, params, fields),
            Stmt::LockGuard {
                body, else_body, ..
            } => {
                collect_backing_from_body(body, params, fields);
                collect_backing_from_body(else_body, params, fields);
            }
            _ => {}
        }
    }
}

fn push_field(name: &str, params: &HashSet<String>, fields: &mut Vec<String>) {
    let lower = name.to_ascii_lowercase();
    if params.contains(&lower) {
        return;
    }
    if !fields.iter().any(|existing| existing == &lower) {
        fields.push(lower);
    }
}

//! AST and token visitors.
//!
//! Default method bodies recurse through child nodes, so an implementor
//! only overrides the node kinds it cares about and calls the matching
//! `walk_*` helper when it still wants the children visited.

use crate::ast::{
    Expr, FunctionDecl, GroupDecl, IfBranch, ImportDecl, Param, PropertyDecl, Script, StateDecl,
    Stmt, StructDecl, TypeName, VariableDecl,
};
use crate::token::Token;

/// Walks a parsed [`Script`].
pub trait Visitor {
    fn visit_script(&mut self, script: &Script) {
        walk_script(self, script);
    }

    fn visit_import(&mut self, import: &ImportDecl) {
        let _ = import;
    }

    fn visit_property(&mut self, property: &PropertyDecl) {
        walk_property(self, property);
    }

    fn visit_variable(&mut self, variable: &VariableDecl) {
        walk_variable(self, variable);
    }

    fn visit_state(&mut self, state: &StateDecl) {
        walk_state(self, state);
    }

    fn visit_function(&mut self, function: &FunctionDecl) {
        walk_function(self, function);
    }

    /// Fallout 4 only: `script.structs` is always empty for a script
    /// parsed in Skyrim mode.
    fn visit_struct(&mut self, struct_decl: &StructDecl) {
        walk_struct(self, struct_decl);
    }

    /// Fallout 4 only: `script.groups` is always empty for a script
    /// parsed in Skyrim mode.
    fn visit_group(&mut self, group: &GroupDecl) {
        walk_group(self, group);
    }

    fn visit_param(&mut self, param: &Param) {
        walk_param(self, param);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        walk_stmt(self, stmt);
    }

    fn visit_if_branch(&mut self, branch: &IfBranch) {
        walk_if_branch(self, branch);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        walk_expr(self, expr);
    }

    fn visit_type_name(&mut self, type_name: &TypeName) {
        let _ = type_name;
    }
}

/// Walks a lexer token stream.
pub trait TokenVisitor {
    fn visit_tokens(&mut self, tokens: &[Token]) {
        walk_tokens(self, tokens);
    }

    fn visit_token(&mut self, token: &Token, index: usize, tokens: &[Token]) {
        let _ = (token, index, tokens);
    }
}

pub fn walk_script<V: Visitor + ?Sized>(visitor: &mut V, script: &Script) {
    for import in &script.imports {
        visitor.visit_import(import);
    }
    for property in &script.properties {
        visitor.visit_property(property);
    }
    for variable in &script.variables {
        visitor.visit_variable(variable);
    }
    for function in &script.functions {
        visitor.visit_function(function);
    }
    for state in &script.states {
        visitor.visit_state(state);
    }
    for struct_decl in &script.structs {
        visitor.visit_struct(struct_decl);
    }
    for group in &script.groups {
        visitor.visit_group(group);
    }
}

pub fn walk_property<V: Visitor + ?Sized>(visitor: &mut V, property: &PropertyDecl) {
    visitor.visit_type_name(&property.type_name);
    if let Some(value) = &property.value {
        visitor.visit_expr(value);
    }
}

pub fn walk_variable<V: Visitor + ?Sized>(visitor: &mut V, variable: &VariableDecl) {
    visitor.visit_type_name(&variable.type_name);
    if let Some(value) = &variable.value {
        visitor.visit_expr(value);
    }
}

pub fn walk_state<V: Visitor + ?Sized>(visitor: &mut V, state: &StateDecl) {
    for function in &state.functions {
        visitor.visit_function(function);
    }
}

pub fn walk_function<V: Visitor + ?Sized>(visitor: &mut V, function: &FunctionDecl) {
    if let Some(return_type) = &function.return_type {
        visitor.visit_type_name(return_type);
    }
    for param in &function.params {
        visitor.visit_param(param);
    }
    for stmt in &function.body {
        visitor.visit_stmt(stmt);
    }
}

pub fn walk_struct<V: Visitor + ?Sized>(visitor: &mut V, struct_decl: &StructDecl) {
    for member in &struct_decl.members {
        visitor.visit_type_name(&member.type_name);
        if let Some(value) = &member.value {
            visitor.visit_expr(value);
        }
    }
}

pub fn walk_group<V: Visitor + ?Sized>(visitor: &mut V, group: &GroupDecl) {
    for property in &group.properties {
        visitor.visit_property(property);
    }
}

pub fn walk_param<V: Visitor + ?Sized>(visitor: &mut V, param: &Param) {
    visitor.visit_type_name(&param.type_name);
    if let Some(default) = &param.default {
        visitor.visit_expr(default);
    }
}

pub fn walk_stmt<V: Visitor + ?Sized>(visitor: &mut V, stmt: &Stmt) {
    match stmt {
        Stmt::VarDecl(variable) => visitor.visit_variable(variable),
        Stmt::Assign { target, value, .. } => {
            visitor.visit_expr(target);
            visitor.visit_expr(value);
        }
        Stmt::Expr { value, .. } => visitor.visit_expr(value),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                visitor.visit_expr(value);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                visitor.visit_if_branch(branch);
            }
            for stmt in else_body {
                visitor.visit_stmt(stmt);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            visitor.visit_expr(condition);
            for stmt in body {
                visitor.visit_stmt(stmt);
            }
        }
    }
}

pub fn walk_if_branch<V: Visitor + ?Sized>(visitor: &mut V, branch: &IfBranch) {
    visitor.visit_expr(&branch.condition);
    for stmt in &branch.body {
        visitor.visit_stmt(stmt);
    }
}

pub fn walk_expr<V: Visitor + ?Sized>(visitor: &mut V, expr: &Expr) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        Expr::Unary { operand, .. } => visitor.visit_expr(operand),
        Expr::Call { callee, args, .. } => {
            visitor.visit_expr(callee);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        Expr::NamedArg { value, .. } => visitor.visit_expr(value),
        Expr::Member { object, .. } => visitor.visit_expr(object),
        Expr::Index { object, index } => {
            visitor.visit_expr(object);
            visitor.visit_expr(index);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => visitor.visit_expr(value),
        Expr::NewArray {
            type_name, size, ..
        } => {
            visitor.visit_type_name(type_name);
            visitor.visit_expr(size);
        }
        Expr::NewStruct { .. } => {}
    }
}

pub fn walk_tokens<V: TokenVisitor + ?Sized>(visitor: &mut V, tokens: &[Token]) {
    for (index, token) in tokens.iter().enumerate() {
        visitor.visit_token(token, index, tokens);
    }
}

#[cfg(test)]
#[path = "visit_tests.rs"]
mod tests;

//! Per-rule visitors and the two walkers the lint registry uses.
//!
//! [`Session`] registers every enabled source-level rule, runs each rule's
//! `check` so the visitor already holds its diagnostics, walks the AST and
//! token stream once, then emits those diagnostics in registration order.
//! Rule `check` functions are unchanged.

use papyrus_parser::ast::{
    Expr, FunctionDecl, IfBranch, ImportDecl, Param, PropertyDecl, Script, StateDecl, Stmt,
    TypeName, VariableDecl,
};
use papyrus_parser::token::Token;
use papyrus_parser::visit::{
    walk_expr, walk_function, walk_if_branch, walk_param, walk_property, walk_script, walk_state,
    walk_stmt, walk_variable, TokenVisitor, Visitor,
};

use crate::config::Config;
use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// Observation-only AST visitor. Methods do not recurse; [`AstWalker`]
/// walks the tree once and fans each node out to every registered lint.
pub trait AstLint {
    fn visit_script(&mut self, script: &Script) {
        let _ = script;
    }
    fn visit_import(&mut self, import: &ImportDecl) {
        let _ = import;
    }
    fn visit_property(&mut self, property: &PropertyDecl) {
        let _ = property;
    }
    fn visit_variable(&mut self, variable: &VariableDecl) {
        let _ = variable;
    }
    fn visit_state(&mut self, state: &StateDecl) {
        let _ = state;
    }
    fn visit_function(&mut self, function: &FunctionDecl) {
        let _ = function;
    }
    fn visit_param(&mut self, param: &Param) {
        let _ = param;
    }
    fn visit_stmt(&mut self, stmt: &Stmt) {
        let _ = stmt;
    }
    fn visit_if_branch(&mut self, branch: &IfBranch) {
        let _ = branch;
    }
    fn visit_expr(&mut self, expr: &Expr) {
        let _ = expr;
    }
    fn visit_type_name(&mut self, type_name: &TypeName) {
        let _ = type_name;
    }
}

/// Observation-only token visitor. [`TokenWalker`] walks the stream once.
pub trait TokenLint {
    fn visit_token(&mut self, token: &Token, index: usize, tokens: &[Token]) {
        let _ = (token, index, tokens);
    }
}

struct NoopAst;

impl AstLint for NoopAst {}

struct NoopTokens;

impl TokenLint for NoopTokens {}

/// A rule's visitor, either over the parsed AST or the token stream.
pub enum LintVisitor {
    Ast(Box<dyn AstLint>),
    Tokens(Box<dyn TokenLint>),
}

impl LintVisitor {
    pub fn ast() -> Self {
        Self::Ast(Box::new(NoopAst))
    }

    pub fn tokens() -> Self {
        Self::Tokens(Box::new(NoopTokens))
    }
}

struct RegisteredAst<E> {
    lint: Box<dyn AstLint>,
    check: CheckFn<E>,
    diagnostics: Vec<Diagnostic>,
}

struct RegisteredTokens<E> {
    lint: Box<dyn TokenLint>,
    check: CheckFn<E>,
    diagnostics: Vec<Diagnostic>,
}

struct RegisteredDirect<E> {
    check: CheckFn<E>,
    diagnostics: Vec<Diagnostic>,
}

type CheckFn<E> =
    Box<dyn FnMut(&str, Option<&Script>, Option<&[Token]>, &Config, &mut E) -> Vec<Diagnostic>>;

#[derive(Clone, Copy)]
enum Item {
    Ast(usize),
    Tokens(usize),
    Direct(usize),
}

/// AST walker: one walk, every registered ast lint notified at each node.
pub struct AstWalker<E> {
    lints: Vec<RegisteredAst<E>>,
}

/// Token walker: one walk, every registered token lint notified at each token.
pub struct TokenWalker<E> {
    lints: Vec<RegisteredTokens<E>>,
}

/// Collects enabled lints into the two walkers, then emits diagnostics.
pub struct Session<E> {
    ast: AstWalker<E>,
    tokens: TokenWalker<E>,
    directs: Vec<RegisteredDirect<E>>,
    order: Vec<Item>,
}

impl<E: ExternalSignatures> Session<E> {
    pub fn new() -> Self {
        Self {
            ast: AstWalker { lints: Vec::new() },
            tokens: TokenWalker { lints: Vec::new() },
            directs: Vec::new(),
            order: Vec::new(),
        }
    }

    pub fn add(
        &mut self,
        visitor: LintVisitor,
        check: impl FnMut(&str, Option<&Script>, Option<&[Token]>, &Config, &mut E) -> Vec<Diagnostic>
            + 'static,
    ) {
        match visitor {
            LintVisitor::Ast(lint) => {
                self.order.push(Item::Ast(self.ast.lints.len()));
                self.ast.lints.push(RegisteredAst {
                    lint,
                    check: Box::new(check),
                    diagnostics: Vec::new(),
                });
            }
            LintVisitor::Tokens(lint) => {
                self.order.push(Item::Tokens(self.tokens.lints.len()));
                self.tokens.lints.push(RegisteredTokens {
                    lint,
                    check: Box::new(check),
                    diagnostics: Vec::new(),
                });
            }
        }
    }

    pub fn add_direct(
        &mut self,
        check: impl FnMut(&str, Option<&Script>, Option<&[Token]>, &Config, &mut E) -> Vec<Diagnostic>
            + 'static,
    ) {
        self.order.push(Item::Direct(self.directs.len()));
        self.directs.push(RegisteredDirect {
            check: Box::new(check),
            diagnostics: Vec::new(),
        });
    }

    pub fn collect(
        mut self,
        source: &str,
        ast: Option<&Script>,
        tokens: Option<&[Token]>,
        config: &Config,
        external: &mut E,
    ) -> Vec<Diagnostic> {
        let order = self.order.clone();
        for item in &order {
            match *item {
                Item::Ast(index) => {
                    let registered = &mut self.ast.lints[index];
                    registered.diagnostics =
                        (registered.check)(source, ast, tokens, config, external);
                }
                Item::Tokens(index) => {
                    let registered = &mut self.tokens.lints[index];
                    registered.diagnostics =
                        (registered.check)(source, ast, tokens, config, external);
                }
                Item::Direct(index) => {
                    let registered = &mut self.directs[index];
                    registered.diagnostics =
                        (registered.check)(source, ast, tokens, config, external);
                }
            }
        }
        self.ast.walk(ast);
        self.tokens.walk(tokens);

        let mut diagnostics = Vec::new();
        for item in order {
            match item {
                Item::Ast(index) => {
                    diagnostics.append(&mut self.ast.lints[index].diagnostics);
                }
                Item::Tokens(index) => {
                    diagnostics.append(&mut self.tokens.lints[index].diagnostics);
                }
                Item::Direct(index) => {
                    diagnostics.append(&mut self.directs[index].diagnostics);
                }
            }
        }
        diagnostics
    }
}

impl<E> AstWalker<E> {
    fn walk(&mut self, ast: Option<&Script>) {
        let Some(script) = ast else {
            return;
        };
        let mut fanout = AstFanout {
            lints: &mut self.lints,
        };
        fanout.visit_script(script);
    }
}

impl<E> TokenWalker<E> {
    fn walk(&mut self, tokens: Option<&[Token]>) {
        let Some(tokens) = tokens else {
            return;
        };
        let mut fanout = TokenFanout {
            lints: &mut self.lints,
        };
        fanout.visit_tokens(tokens);
    }
}

struct AstFanout<'a, E> {
    lints: &'a mut [RegisteredAst<E>],
}

impl<E> Visitor for AstFanout<'_, E> {
    fn visit_script(&mut self, script: &Script) {
        for registered in &mut *self.lints {
            registered.lint.visit_script(script);
        }
        walk_script(self, script);
    }

    fn visit_import(&mut self, import: &ImportDecl) {
        for registered in &mut *self.lints {
            registered.lint.visit_import(import);
        }
    }

    fn visit_property(&mut self, property: &PropertyDecl) {
        for registered in &mut *self.lints {
            registered.lint.visit_property(property);
        }
        walk_property(self, property);
    }

    fn visit_variable(&mut self, variable: &VariableDecl) {
        for registered in &mut *self.lints {
            registered.lint.visit_variable(variable);
        }
        walk_variable(self, variable);
    }

    fn visit_state(&mut self, state: &StateDecl) {
        for registered in &mut *self.lints {
            registered.lint.visit_state(state);
        }
        walk_state(self, state);
    }

    fn visit_function(&mut self, function: &FunctionDecl) {
        for registered in &mut *self.lints {
            registered.lint.visit_function(function);
        }
        walk_function(self, function);
    }

    fn visit_param(&mut self, param: &Param) {
        for registered in &mut *self.lints {
            registered.lint.visit_param(param);
        }
        walk_param(self, param);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        for registered in &mut *self.lints {
            registered.lint.visit_stmt(stmt);
        }
        walk_stmt(self, stmt);
    }

    fn visit_if_branch(&mut self, branch: &IfBranch) {
        for registered in &mut *self.lints {
            registered.lint.visit_if_branch(branch);
        }
        walk_if_branch(self, branch);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        for registered in &mut *self.lints {
            registered.lint.visit_expr(expr);
        }
        walk_expr(self, expr);
    }

    fn visit_type_name(&mut self, type_name: &TypeName) {
        for registered in &mut *self.lints {
            registered.lint.visit_type_name(type_name);
        }
    }
}

struct TokenFanout<'a, E> {
    lints: &'a mut [RegisteredTokens<E>],
}

impl<E> TokenVisitor for TokenFanout<'_, E> {
    fn visit_token(&mut self, token: &Token, index: usize, tokens: &[Token]) {
        for registered in &mut *self.lints {
            registered.lint.visit_token(token, index, tokens);
        }
    }
}

#[cfg(test)]
#[path = "visitor_tests.rs"]
mod tests;

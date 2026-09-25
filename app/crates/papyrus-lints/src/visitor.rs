//! Per-rule visitors and the two walkers the lint registry uses.
//!
//! Each visitor owns a [`Store`] of diagnostics collected during the walk.
//! [`Session`] walks AST and tokens once, then drains those stores.
//! [`run`] is the same path for a single rule — that is what each visitor
//! rule's `check` calls. Rules implement [`AstLint`] or [`TokenLint`] and
//! emit from the matching `visit_*` callbacks instead of re-scanning the
//! file inside `check`.

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

/// Diagnostics collected by one visitor during a walk. `check` returns
/// [`Store::take`] after the walk finishes.
#[derive(Debug, Default)]
pub struct Store {
    issues: Vec<Diagnostic>,
}

impl Store {
    pub fn emit(
        &mut self,
        line: usize,
        column: usize,
        message: impl Into<String>,
        rule: &'static str,
    ) {
        self.issues.push(Diagnostic {
            line,
            column,
            message: message.into(),
            rule,
        });
    }

    #[allow(dead_code)]
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.issues.push(diagnostic);
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.issues.extend(diagnostics);
    }

    pub fn take(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.issues)
    }
}

/// Context passed to every visitor callback during a walk. Diagnostics go
/// on the visitor's [`Store`], not here.
pub struct VisitCtx<'a> {
    pub source: &'a str,
    pub ast: Option<&'a Script>,
    pub tokens: Option<&'a [Token]>,
    pub config: &'a Config,
    pub external: &'a mut dyn ExternalSignatures,
    /// Line of the node currently being visited (1-indexed). Expression
    /// callbacks inherit the enclosing statement's line.
    pub line: usize,
}

/// Observation-only AST visitor. Methods do not recurse; [`AstWalker`]
/// walks the tree once and fans each node out to every registered lint.
pub trait AstLint {
    fn store(&mut self) -> &mut Store;

    fn take_issues(&mut self) -> Vec<Diagnostic> {
        self.store().take()
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        let _ = ctx;
    }
    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        let _ = (script, ctx);
    }
    fn visit_import(&mut self, import: &ImportDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (import, ctx);
    }
    fn visit_property(&mut self, property: &PropertyDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (property, ctx);
    }
    fn visit_variable(&mut self, variable: &VariableDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (variable, ctx);
    }
    fn visit_state(&mut self, state: &StateDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (state, ctx);
    }
    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (function, ctx);
    }
    fn leave_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        let _ = (function, ctx);
    }
    fn visit_param(&mut self, param: &Param, ctx: &mut VisitCtx<'_>) {
        let _ = (param, ctx);
    }
    fn visit_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let _ = (stmt, ctx);
    }
    fn leave_stmt(&mut self, stmt: &Stmt, ctx: &mut VisitCtx<'_>) {
        let _ = (stmt, ctx);
    }
    fn visit_if_branch(&mut self, branch: &IfBranch, ctx: &mut VisitCtx<'_>) {
        let _ = (branch, ctx);
    }
    fn leave_if_branch(&mut self, branch: &IfBranch, ctx: &mut VisitCtx<'_>) {
        let _ = (branch, ctx);
    }
    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let _ = (expr, ctx);
    }
    fn leave_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let _ = (expr, ctx);
    }
    fn visit_type_name(&mut self, type_name: &TypeName, ctx: &mut VisitCtx<'_>) {
        let _ = (type_name, ctx);
    }
    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        let _ = ctx;
    }
}

/// Observation-only token visitor. [`TokenWalker`] walks the stream once.
pub trait TokenLint {
    fn store(&mut self) -> &mut Store;

    fn take_issues(&mut self) -> Vec<Diagnostic> {
        self.store().take()
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        let _ = ctx;
    }
    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        ctx: &mut VisitCtx<'_>,
    ) {
        let _ = (token, index, tokens, ctx);
    }
    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        let _ = ctx;
    }
}

/// A rule's visitor, either over the parsed AST or the token stream.
pub enum LintVisitor {
    Ast(Box<dyn AstLint>),
    Tokens(Box<dyn TokenLint>),
}

struct RegisteredAst {
    lint: Box<dyn AstLint>,
    diagnostics: Vec<Diagnostic>,
}

struct RegisteredTokens {
    lint: Box<dyn TokenLint>,
    diagnostics: Vec<Diagnostic>,
}

struct RegisteredDirect<E> {
    #[allow(clippy::type_complexity)]
    check:
        Box<dyn FnMut(&str, Option<&Script>, Option<&[Token]>, &Config, &mut E) -> Vec<Diagnostic>>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy)]
enum Item {
    Ast(usize),
    Tokens(usize),
    Direct(usize),
}

/// AST walker: one walk, every registered ast lint notified at each node.
#[derive(Default)]
pub struct AstWalker {
    lints: Vec<RegisteredAst>,
}

/// Token walker: one walk, every registered token lint notified at each token.
#[derive(Default)]
pub struct TokenWalker {
    lints: Vec<RegisteredTokens>,
}

/// Collects enabled lints into the two walkers, then emits diagnostics.
pub struct Session<E> {
    ast: AstWalker,
    tokens: TokenWalker,
    directs: Vec<RegisteredDirect<E>>,
    order: Vec<Item>,
}

impl<E: ExternalSignatures> Session<E> {
    pub fn new() -> Self {
        Self {
            ast: AstWalker::default(),
            tokens: TokenWalker::default(),
            directs: Vec::new(),
            order: Vec::new(),
        }
    }

    pub fn add(&mut self, visitor: LintVisitor) {
        match visitor {
            LintVisitor::Ast(lint) => {
                self.order.push(Item::Ast(self.ast.lints.len()));
                self.ast.lints.push(RegisteredAst {
                    lint,
                    diagnostics: Vec::new(),
                });
            }
            LintVisitor::Tokens(lint) => {
                self.order.push(Item::Tokens(self.tokens.lints.len()));
                self.tokens.lints.push(RegisteredTokens {
                    lint,
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
        {
            let mut ctx = WalkCtx {
                source,
                ast,
                tokens,
                config,
                external,
            };
            self.ast.begin(&mut ctx);
            self.tokens.begin(&mut ctx);
            self.ast.walk(&mut ctx);
            self.tokens.walk(&mut ctx);
            self.ast.finish(&mut ctx);
            self.tokens.finish(&mut ctx);
        }

        for registered in &mut self.ast.lints {
            registered.diagnostics = registered.lint.take_issues();
        }
        for registered in &mut self.tokens.lints {
            registered.diagnostics = registered.lint.take_issues();
        }

        let order = self.order.clone();
        for item in &order {
            if let Item::Direct(index) = *item {
                let registered = &mut self.directs[index];
                registered.diagnostics = (registered.check)(source, ast, tokens, config, external);
            }
        }

        let mut diagnostics = Vec::new();
        for item in order {
            match item {
                Item::Ast(index) => diagnostics.append(&mut self.ast.lints[index].diagnostics),
                Item::Tokens(index) => {
                    diagnostics.append(&mut self.tokens.lints[index].diagnostics)
                }
                Item::Direct(index) => diagnostics.append(&mut self.directs[index].diagnostics),
            }
        }
        diagnostics
    }
}

/// Runs a single rule visitor through the same walk [`Session`] uses.
pub fn run<E: ExternalSignatures>(
    visitor: LintVisitor,
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[Token]>,
    config: &Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    let mut session = Session::new();
    session.add(visitor);
    session.collect(source, ast, tokens, config, external)
}

fn stmt_line(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(variable) => variable.line,
        Stmt::Assign { line, .. }
        | Stmt::Expr { line, .. }
        | Stmt::Return { line, .. }
        | Stmt::If { line, .. }
        | Stmt::While { line, .. }
        | Stmt::LockGuard { line, .. } => *line,
    }
}

struct WalkCtx<'a, E> {
    source: &'a str,
    ast: Option<&'a Script>,
    tokens: Option<&'a [Token]>,
    config: &'a Config,
    external: &'a mut E,
}

impl AstWalker {
    fn begin<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, 1, |lint, ctx| lint.begin(ctx));
    }

    fn finish<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, 1, |lint, ctx| lint.finish(ctx));
    }

    fn walk<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        let Some(script) = ctx.ast else {
            return;
        };
        let mut fanout = AstFanout {
            lints: &mut self.lints,
            source: ctx.source,
            ast: ctx.ast,
            tokens: ctx.tokens,
            config: ctx.config,
            external: ctx.external,
            line: script.line,
        };
        fanout.visit_script(script);
    }

    fn notify<E: ExternalSignatures>(
        &mut self,
        ctx: &mut WalkCtx<'_, E>,
        line: usize,
        mut f: impl FnMut(&mut dyn AstLint, &mut VisitCtx<'_>),
    ) {
        let source = ctx.source;
        let ast = ctx.ast;
        let tokens = ctx.tokens;
        let config = ctx.config;
        let external: &mut dyn ExternalSignatures = ctx.external;
        let mut visit = VisitCtx {
            source,
            ast,
            tokens,
            config,
            external,
            line,
        };
        for registered in &mut self.lints {
            f(&mut *registered.lint, &mut visit);
        }
    }
}

impl TokenWalker {
    fn begin<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, |lint, ctx| lint.begin(ctx));
    }

    fn finish<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, |lint, ctx| lint.finish(ctx));
    }

    fn walk<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        let Some(tokens) = ctx.tokens else {
            return;
        };
        let mut fanout = TokenFanout {
            lints: &mut self.lints,
            source: ctx.source,
            ast: ctx.ast,
            tokens: ctx.tokens,
            config: ctx.config,
            external: ctx.external,
        };
        fanout.visit_tokens(tokens);
    }

    fn notify<E: ExternalSignatures>(
        &mut self,
        ctx: &mut WalkCtx<'_, E>,
        mut f: impl FnMut(&mut dyn TokenLint, &mut VisitCtx<'_>),
    ) {
        let source = ctx.source;
        let ast = ctx.ast;
        let tokens = ctx.tokens;
        let config = ctx.config;
        let external: &mut dyn ExternalSignatures = ctx.external;
        let mut visit = VisitCtx {
            source,
            ast,
            tokens,
            config,
            external,
            line: 1,
        };
        for registered in &mut self.lints {
            f(&mut *registered.lint, &mut visit);
        }
    }
}

struct AstFanout<'a> {
    lints: &'a mut [RegisteredAst],
    source: &'a str,
    ast: Option<&'a Script>,
    tokens: Option<&'a [Token]>,
    config: &'a Config,
    external: &'a mut dyn ExternalSignatures,
    line: usize,
}

impl AstFanout<'_> {
    fn notify(&mut self, mut f: impl FnMut(&mut dyn AstLint, &mut VisitCtx<'_>)) {
        let source = self.source;
        let ast = self.ast;
        let tokens = self.tokens;
        let config = self.config;
        let external = &mut *self.external;
        let line = self.line;
        let mut ctx = VisitCtx {
            source,
            ast,
            tokens,
            config,
            external,
            line,
        };
        for registered in &mut *self.lints {
            f(&mut *registered.lint, &mut ctx);
        }
    }
}

impl Visitor for AstFanout<'_> {
    fn visit_script(&mut self, script: &Script) {
        self.line = script.line;
        self.notify(|lint, ctx| lint.visit_script(script, ctx));
        walk_script(self, script);
    }

    fn visit_import(&mut self, import: &ImportDecl) {
        self.line = import.line;
        self.notify(|lint, ctx| lint.visit_import(import, ctx));
    }

    fn visit_property(&mut self, property: &PropertyDecl) {
        self.line = property.line;
        self.notify(|lint, ctx| lint.visit_property(property, ctx));
        walk_property(self, property);
    }

    fn visit_variable(&mut self, variable: &VariableDecl) {
        self.line = variable.line;
        self.notify(|lint, ctx| lint.visit_variable(variable, ctx));
        walk_variable(self, variable);
    }

    fn visit_state(&mut self, state: &StateDecl) {
        self.line = state.line;
        self.notify(|lint, ctx| lint.visit_state(state, ctx));
        walk_state(self, state);
    }

    fn visit_function(&mut self, function: &FunctionDecl) {
        self.line = function.line;
        self.notify(|lint, ctx| lint.visit_function(function, ctx));
        walk_function(self, function);
        self.notify(|lint, ctx| lint.leave_function(function, ctx));
    }

    fn visit_param(&mut self, param: &Param) {
        self.notify(|lint, ctx| lint.visit_param(param, ctx));
        walk_param(self, param);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        self.line = stmt_line(stmt);
        self.notify(|lint, ctx| lint.visit_stmt(stmt, ctx));
        walk_stmt(self, stmt);
        self.notify(|lint, ctx| lint.leave_stmt(stmt, ctx));
    }

    fn visit_if_branch(&mut self, branch: &IfBranch) {
        self.line = branch.line;
        self.notify(|lint, ctx| lint.visit_if_branch(branch, ctx));
        walk_if_branch(self, branch);
        self.notify(|lint, ctx| lint.leave_if_branch(branch, ctx));
    }

    fn visit_expr(&mut self, expr: &Expr) {
        self.notify(|lint, ctx| lint.visit_expr(expr, ctx));
        walk_expr(self, expr);
        self.notify(|lint, ctx| lint.leave_expr(expr, ctx));
    }

    fn visit_type_name(&mut self, type_name: &TypeName) {
        self.notify(|lint, ctx| lint.visit_type_name(type_name, ctx));
    }
}

struct TokenFanout<'a> {
    lints: &'a mut [RegisteredTokens],
    source: &'a str,
    ast: Option<&'a Script>,
    tokens: Option<&'a [Token]>,
    config: &'a Config,
    external: &'a mut dyn ExternalSignatures,
}

impl TokenFanout<'_> {
    fn notify(&mut self, mut f: impl FnMut(&mut dyn TokenLint, &mut VisitCtx<'_>)) {
        let source = self.source;
        let ast = self.ast;
        let tokens = self.tokens;
        let config = self.config;
        let external = &mut *self.external;
        let mut ctx = VisitCtx {
            source,
            ast,
            tokens,
            config,
            external,
            line: 1,
        };
        for registered in &mut *self.lints {
            f(&mut *registered.lint, &mut ctx);
        }
    }
}

impl TokenVisitor for TokenFanout<'_> {
    fn visit_token(&mut self, token: &Token, index: usize, tokens: &[Token]) {
        self.notify(|lint, ctx| lint.visit_token(token, index, tokens, ctx));
    }
}

#[cfg(test)]
#[path = "visitor_tests.rs"]
mod tests;

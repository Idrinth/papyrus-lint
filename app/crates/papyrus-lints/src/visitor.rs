//! Per-rule visitors and the two walkers the lint registry uses.
//!
//! [`Session`] registers every enabled source-level rule onto an AST walker
//! or a token walker, walks each tree once, and emits the diagnostics those
//! visitors collected. [`run`] is the same path for a single rule — that is
//! what each visitor rule's `check` calls.

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

/// Context passed to every visitor callback during a walk.
pub struct VisitCtx<'a> {
    pub source: &'a str,
    pub ast: Option<&'a Script>,
    pub tokens: Option<&'a [Token]>,
    pub config: &'a Config,
    pub external: &'a mut dyn ExternalSignatures,
    pub diagnostics: &'a mut Vec<Diagnostic>,
}

impl VisitCtx<'_> {
    #[allow(dead_code)]
    pub fn emit(&mut self, line: usize, column: usize, message: String, rule: &'static str) {
        self.diagnostics.push(Diagnostic {
            line,
            column,
            message,
            rule,
        });
    }
}

/// Observation-only AST visitor. Methods do not recurse; [`AstWalker`]
/// walks the tree once and fans each node out to every registered lint.
pub trait AstLint {
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
    fn visit_if_branch(&mut self, branch: &IfBranch, ctx: &mut VisitCtx<'_>) {
        let _ = (branch, ctx);
    }
    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
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

/// Collects diagnostics from a full-script `check` during [`AstLint::visit_script`].
pub fn from_ast(
    collect: impl Fn(
            &str,
            Option<&Script>,
            Option<&[Token]>,
            &Config,
            &mut dyn ExternalSignatures,
        ) -> Vec<Diagnostic>
        + 'static,
) -> LintVisitor {
    struct Collect<F>(F);
    impl<
            F: Fn(
                &str,
                Option<&Script>,
                Option<&[Token]>,
                &Config,
                &mut dyn ExternalSignatures,
            ) -> Vec<Diagnostic>,
        > AstLint for Collect<F>
    {
        fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
            ctx.diagnostics.extend((self.0)(
                ctx.source,
                Some(script),
                ctx.tokens,
                ctx.config,
                ctx.external,
            ));
        }

        fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
            if ctx.ast.is_none() {
                ctx.diagnostics.extend((self.0)(
                    ctx.source,
                    None,
                    ctx.tokens,
                    ctx.config,
                    ctx.external,
                ));
            }
        }
    }
    LintVisitor::Ast(Box::new(Collect(collect)))
}

/// Collects diagnostics from a full-stream `check` at the start of the token walk.
pub fn from_tokens(
    collect: impl Fn(
            &str,
            Option<&Script>,
            Option<&[Token]>,
            &Config,
            &mut dyn ExternalSignatures,
        ) -> Vec<Diagnostic>
        + 'static,
) -> LintVisitor {
    struct Collect<F>(F);
    impl<
            F: Fn(
                &str,
                Option<&Script>,
                Option<&[Token]>,
                &Config,
                &mut dyn ExternalSignatures,
            ) -> Vec<Diagnostic>,
        > TokenLint for Collect<F>
    {
        fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
            ctx.diagnostics.extend((self.0)(
                ctx.source,
                ctx.ast,
                ctx.tokens,
                ctx.config,
                ctx.external,
            ));
        }
    }
    LintVisitor::Tokens(Box::new(Collect(collect)))
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

struct WalkCtx<'a, E> {
    source: &'a str,
    ast: Option<&'a Script>,
    tokens: Option<&'a [Token]>,
    config: &'a Config,
    external: &'a mut E,
}

impl AstWalker {
    fn begin<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, |lint, ctx| lint.begin(ctx));
    }

    fn finish<E: ExternalSignatures>(&mut self, ctx: &mut WalkCtx<'_, E>) {
        self.notify(ctx, |lint, ctx| lint.finish(ctx));
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
        };
        fanout.visit_script(script);
    }

    fn notify<E: ExternalSignatures>(
        &mut self,
        ctx: &mut WalkCtx<'_, E>,
        mut f: impl FnMut(&mut dyn AstLint, &mut VisitCtx<'_>),
    ) {
        let source = ctx.source;
        let ast = ctx.ast;
        let tokens = ctx.tokens;
        let config = ctx.config;
        let external: &mut dyn ExternalSignatures = ctx.external;
        for registered in &mut self.lints {
            let mut visit = VisitCtx {
                source,
                ast,
                tokens,
                config,
                external,
                diagnostics: &mut registered.diagnostics,
            };
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
        for registered in &mut self.lints {
            let mut visit = VisitCtx {
                source,
                ast,
                tokens,
                config,
                external,
                diagnostics: &mut registered.diagnostics,
            };
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
}

impl AstFanout<'_> {
    fn notify(&mut self, mut f: impl FnMut(&mut dyn AstLint, &mut VisitCtx<'_>)) {
        let source = self.source;
        let ast = self.ast;
        let tokens = self.tokens;
        let config = self.config;
        let external = &mut *self.external;
        for registered in &mut *self.lints {
            let mut ctx = VisitCtx {
                source,
                ast,
                tokens,
                config,
                external,
                diagnostics: &mut registered.diagnostics,
            };
            f(&mut *registered.lint, &mut ctx);
        }
    }
}

impl Visitor for AstFanout<'_> {
    fn visit_script(&mut self, script: &Script) {
        self.notify(|lint, ctx| lint.visit_script(script, ctx));
        walk_script(self, script);
    }

    fn visit_import(&mut self, import: &ImportDecl) {
        self.notify(|lint, ctx| lint.visit_import(import, ctx));
    }

    fn visit_property(&mut self, property: &PropertyDecl) {
        self.notify(|lint, ctx| lint.visit_property(property, ctx));
        walk_property(self, property);
    }

    fn visit_variable(&mut self, variable: &VariableDecl) {
        self.notify(|lint, ctx| lint.visit_variable(variable, ctx));
        walk_variable(self, variable);
    }

    fn visit_state(&mut self, state: &StateDecl) {
        self.notify(|lint, ctx| lint.visit_state(state, ctx));
        walk_state(self, state);
    }

    fn visit_function(&mut self, function: &FunctionDecl) {
        self.notify(|lint, ctx| lint.visit_function(function, ctx));
        walk_function(self, function);
        self.notify(|lint, ctx| lint.leave_function(function, ctx));
    }

    fn visit_param(&mut self, param: &Param) {
        self.notify(|lint, ctx| lint.visit_param(param, ctx));
        walk_param(self, param);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        self.notify(|lint, ctx| lint.visit_stmt(stmt, ctx));
        walk_stmt(self, stmt);
    }

    fn visit_if_branch(&mut self, branch: &IfBranch) {
        self.notify(|lint, ctx| lint.visit_if_branch(branch, ctx));
        walk_if_branch(self, branch);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        self.notify(|lint, ctx| lint.visit_expr(expr, ctx));
        walk_expr(self, expr);
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
        for registered in &mut *self.lints {
            let mut ctx = VisitCtx {
                source,
                ast,
                tokens,
                config,
                external,
                diagnostics: &mut registered.diagnostics,
            };
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

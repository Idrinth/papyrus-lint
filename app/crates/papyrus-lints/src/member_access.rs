//! Enforces the `@public`, `@protected`, and `@private` access levels on
//! functions and properties resolved from project scripts.

use papyrus_parser::ast::{AccessLevel, Expr, FunctionDecl, Script, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::external_signatures::MemberAccess;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

pub const RULE: &str = "member-access";

#[derive(Default)]
struct Collect {
    store: Store,
    script_name: String,
    env: Option<TypeEnv>,
    statement_line: usize,
    called_member: Option<(String, String)>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.script_name.clone_from(&script.name);
        self.env = Some(TypeEnv::for_script(script));
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.enter_function(function);
        }
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        self.statement_line = match stmt {
            Stmt::VarDecl(decl) => decl.line,
            Stmt::Assign { line, .. }
            | Stmt::Expr { line, .. }
            | Stmt::Return { line, .. }
            | Stmt::If { line, .. }
            | Stmt::While { line, .. }
            | Stmt::LockGuard { line, .. } => *line,
        };
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Some(env) = self.env.as_ref() else { return };
        match expr {
            Expr::Call { callee, line, col, .. } => {
                let (target_type, member) = match callee.as_ref() {
                    Expr::Member { object, property } => {
                        let Some(target) = target_type(object, env) else { return };
                        (target, property.as_str())
                    }
                    Expr::Identifier(name) => (self.script_name.clone(), name.as_str()),
                    _ => return,
                };
                let access = ctx.external.function_access(&target_type, member);
                self.report_if_denied(access, member, "function", *line, *col, ctx);
                self.called_member = Some((target_type, member.to_ascii_lowercase()));
            }
            Expr::Member { object, property } => {
                let Some(target_type) = target_type(object, env) else { return };
                if self
                    .called_member
                    .as_ref()
                    .is_some_and(|(called_type, called_name)| {
                        called_type.eq_ignore_ascii_case(&target_type)
                            && called_name.eq_ignore_ascii_case(property)
                    })
                {
                    self.called_member = None;
                    return;
                }
                let access = ctx.external.property_access(&target_type, property);
                self.report_if_denied(access, property, "property", self.statement_line, 1, ctx);
            }
            _ => {}
        }
    }
}

impl Collect {
    fn report_if_denied(
        &mut self,
        access: Option<MemberAccess>,
        member: &str,
        kind: &str,
        line: usize,
        column: usize,
        ctx: &mut VisitCtx<'_>,
    ) {
        let Some(access) = access else { return };
        let allowed = match access.access_level {
            AccessLevel::Public => true,
            AccessLevel::Private => self.script_name.eq_ignore_ascii_case(&access.declaring_type),
            AccessLevel::Protected => ctx
                .external
                .is_subtype(&self.script_name, &access.declaring_type),
        };
        if !allowed {
            let level = match access.access_level {
                AccessLevel::Private => "private",
                AccessLevel::Protected => "protected",
                AccessLevel::Public => unreachable!(),
            };
            self.store.emit(
                line,
                column,
                format!(
                    "[error] {kind} '{member}' is @{level} on '{}' and cannot be accessed from '{}'",
                    access.declaring_type, self.script_name
                ),
                RULE,
            );
        }
    }
}

fn target_type(object: &Expr, env: &TypeEnv) -> Option<String> {
    infer_type(object, env).map(|type_name| type_name.name).or_else(|| {
        if let Expr::Identifier(name) = object {
            (env.lookup(name).is_none()).then(|| name.clone())
        } else {
            None
        }
    })
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

#[allow(dead_code)]
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

#[cfg(test)]
#[path = "member_access_tests.rs"]
mod tests;

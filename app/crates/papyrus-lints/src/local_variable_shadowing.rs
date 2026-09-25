//! Flags local variables that shadow a `Property` or a plain script-level
//! variable (field) declared on the same script or on a parent script,
//! since reading the name inside the function then reads the local rather
//! than the property/field, which is a common source of confusion (and,
//! once the local goes out of scope conceptually, bugs).
//!
//! Like [`crate::argument_types`], this works from the parsed AST rather
//! than raw tokens, and reuses that module's
//! [`external_signatures::ExternalSignatures`] trait so a caller that can
//! resolve other scripts (e.g. the desktop app's `FunctionTable`) can also
//! check shadowing against a parent script's properties, not just the
//! linted script's own. A script that doesn't parse cleanly is left
//! unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{Script, VariableDecl};

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "local-variable-shadowing";

#[derive(Default)]
struct Collect {
    store: Store,
    protected: Vec<bool>,
    own_properties: HashSet<String>,
    own_variables: HashSet<String>,
    extends: Option<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.own_properties = script
            .properties
            .iter()
            .map(|property| property.name.to_ascii_lowercase())
            .collect();
        self.own_variables = script
            .variables
            .iter()
            .map(|variable| variable.name.to_ascii_lowercase())
            .collect();
        self.extends = script.extends.clone();
    }

    fn visit_variable(&mut self, decl: &VariableDecl, ctx: &mut VisitCtx<'_>) {
        let Some(script) = ctx.ast else {
            return;
        };
        if script
            .variables
            .iter()
            .any(|declared| std::ptr::eq(declared, decl))
        {
            return;
        }
        if self.protected.get(decl.line).copied().unwrap_or(false) {
            return;
        }
        if let Some(diagnostic) = check_decl(
            decl,
            self.extends.as_deref(),
            &self.own_properties,
            &self.own_variables,
            ctx.external,
        ) {
            self.store.push(diagnostic);
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for local variables that shadow a property or field
/// declared on the same script. Shadowing a property or field declared on
/// a parent script isn't checked this way, since resolving parent scripts
/// to files requires filesystem access this crate deliberately doesn't
/// have; see [`check_with`] for that. Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged, since it's generated boilerplate the user can't edit.
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

/// Like [`check`], but also flags a local variable that shadows a property
/// or field declared on a parent script, resolved (including through
/// `Extends`) through `external`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures + ?Sized>(
    source: &str,
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let own_properties: HashSet<String> = script
        .properties
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    let own_variables: HashSet<String> = script
        .variables
        .iter()
        .map(|v| v.name.to_ascii_lowercase())
        .collect();

    let mut diagnostics = Vec::new();
    for function in script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    ) {
        for decl in collect_var_decls(&function.body) {
            if protected.get(decl.line).copied().unwrap_or(false) {
                continue;
            }
            diagnostics.extend(check_decl(
                decl,
                script.extends.as_deref(),
                &own_properties,
                &own_variables,
                external,
            ));
        }
    }
    diagnostics
}

fn check_decl<E: ExternalSignatures + ?Sized>(
    decl: &VariableDecl,
    extends: Option<&str>,
    own_properties: &HashSet<String>,
    own_variables: &HashSet<String>,
    external: &mut E,
) -> Option<Diagnostic> {
    let name_lower = decl.name.to_ascii_lowercase();

    if own_properties.contains(&name_lower) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows this script's own property '{}'",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    if own_variables.contains(&name_lower) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows this script's own variable '{}'",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    let parent = extends?;
    if external.has_property(parent, &decl.name) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows a property '{}' inherited from a parent script",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    if external.has_field(parent, &decl.name) {
        return Some(Diagnostic {
            line: decl.line,
            column: 1,
            message: format!(
                "[warning] Local variable '{}' shadows a variable '{}' inherited from a parent script",
                decl.name, decl.name
            ),
            rule: RULE,
        });
    }

    None
}

/// Finds every `VariableDecl` in `body`, including ones nested inside
/// `If`/`ElseIf`/`Else` branches and `While` bodies, since Papyrus locals
/// aren't block-scoped.
fn collect_var_decls(body: &[papyrus_parser::ast::Stmt]) -> Vec<&VariableDecl> {
    let mut decls = Vec::new();
    for stmt in body {
        match stmt {
            papyrus_parser::ast::Stmt::VarDecl(decl) => decls.push(decl),
            papyrus_parser::ast::Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    decls.extend(collect_var_decls(&branch.body));
                }
                decls.extend(collect_var_decls(else_body));
            }
            papyrus_parser::ast::Stmt::While { body, .. } => {
                decls.extend(collect_var_decls(body))
            }
            papyrus_parser::ast::Stmt::LockGuard { body, else_body, .. } => {
                decls.extend(collect_var_decls(body));
                decls.extend(collect_var_decls(else_body));
            }
            _ => {}
        }
    }
    decls
}

#[cfg(test)]
#[path = "local_variable_shadowing_tests.rs"]
mod tests;

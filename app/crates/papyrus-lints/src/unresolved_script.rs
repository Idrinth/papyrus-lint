//! Flags an unresolved parent script, declared type, or call through
//! Papyrus's static/global call syntax
//! (`ScriptName.Function(...)`, e.g. `Utility.Wait(1.0)` or
//! `MyMissingScript.DoThing()`) whose target script can't be located,
//! since Papyrus resolves that name against a script file at compile time
//! and a call through a script that doesn't exist can never compile.
//!
//! Only a call whose object is a bare identifier not already known as a
//! local variable, parameter, or property (i.e. definitely not an
//! instance the script already has a handle to) is treated as a script
//! reference at all — anything resolvable locally is left to the
//! "Argument type check"/"Return type check" lints instead. Whether such a
//! name, an `Extends` parent, or a type annotation can be located depends
//! on the project's own scripts and built-in native type data,
//! neither of which this crate has access to on its own; a caller that can
//! resolve them (e.g. the desktop app's `FunctionTable`) does so by
//! implementing [`ExternalSignatures::script_exists`] and
//! [`ExternalSignatures::type_exists`] and calling
//! [`check_with`] instead of [`check`].
//!
//! A bare name that is a `Struct` on the script being linted, on an
//! ancestor it `Extends`, or declared directly on a script it `Import`s is
//! a type, not a missing script. Qualified `Script:Struct` names stay on
//! [`ExternalSignatures::type_exists`]. Namespaced scripts (`namespace:Script`)
//! are not resolved; they are left unflagged rather than treated as a
//! missing `Script:Struct`.

use papyrus_parser::ast::{Expr, FunctionDecl, Script, TypeName};
use papyrus_parser::types::TypeEnv;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unresolved-script";

#[derive(Default)]
struct Collect {
    store: Store,
    env: Option<TypeEnv>,
    /// Lowercased `Struct` names declared on the script being linted.
    local_structs: Vec<String>,
    parent: Option<String>,
    imports: Vec<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
        self.local_structs = script
            .structs
            .iter()
            .map(|struct_decl| struct_decl.name.to_ascii_lowercase())
            .collect();
        self.parent = script.extends.clone();
        self.imports = script
            .imports
            .iter()
            .map(|import| import.name.clone())
            .collect();
        if let Some(parent) = &script.extends {
            if !ctx.external.type_exists(parent)
                && !is_unsupported_namespace(ctx.external, parent)
            {
                self.store
                    .push(missing_type(script.line, 1, parent, "Parent script"));
            }
        }
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

    fn visit_type_name(&mut self, type_name: &TypeName, ctx: &mut VisitCtx<'_>) {
        self.note_unresolved_type(ctx, &type_name.name);
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        match expr {
            Expr::Call {
                callee, line, col, ..
            } => {
                let Expr::Member { object, .. } = &**callee else {
                    return;
                };
                let Expr::Identifier(name) = &**object else {
                    return;
                };
                let Some(env) = self.env.as_ref() else {
                    return;
                };
                if env.lookup(name).is_none()
                    && !ctx.external.script_exists(name)
                    && !is_unsupported_namespace(ctx.external, name)
                {
                    self.store.push(missing(*line, *col, name));
                }
            }
            Expr::Cast { type_name, .. } | Expr::Is { type_name, .. } => {
                self.note_unresolved_type(ctx, type_name);
            }
            _ => {}
        }
    }
}

impl Collect {
    fn note_unresolved_type(&mut self, ctx: &mut VisitCtx<'_>, name: &str) {
        let name = array_element_name(name);
        if self.is_local_struct(name) || ctx.external.type_exists(name) {
            return;
        }
        // `type_exists` covers primitives, scripts, arrays, and qualified
        // `Script:Struct` names. A bare name it rejects is still a struct
        // when an `Extends` ancestor declares it, or an `Import` declares
        // that struct itself (not a struct on the import's parent).
        if self.inherited_or_imported_struct(ctx, name) {
            return;
        }
        if is_unsupported_namespace(ctx.external, name) {
            return;
        }
        self.store.push(missing_type(ctx.line, 1, name, "Type"));
    }

    fn is_local_struct(&self, name: &str) -> bool {
        if name.contains(':') {
            return false;
        }
        let key = name.to_ascii_lowercase();
        self.local_structs
            .iter()
            .any(|struct_name| struct_name == &key)
    }

    fn inherited_or_imported_struct(&self, ctx: &mut VisitCtx<'_>, name: &str) -> bool {
        if name.contains(':') {
            return false;
        }
        if let Some(parent) = &self.parent {
            if ctx.external.declares_struct_in_ancestry(parent, name) {
                return true;
            }
        }
        self.imports
            .iter()
            .any(|import| ctx.external.declares_struct(import, name))
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for calls through a script name that can't be
/// resolved. Since this crate has no filesystem access on its own, no
/// script can ever be confirmed missing this way; see [`check_with`] to
/// actually resolve script names.
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

/// Like [`check`], but resolves each call's target script through
/// `external`, flagging one that can't be located.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    crate::visitor::run(
        visitor(),
        "",
        ast,
        None,
        &crate::config::Config::default(),
        external,
    )
}

fn missing_type(line: usize, col: usize, name: &str, kind: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] {kind} '{name}' could not be located"),
        rule: RULE,
    }
}

/// `T[]` is an array of `T`, never a script whose name includes the brackets.
/// Cast and `is` expressions store that spelling in one string; declarations
/// keep the brackets on [`TypeName::is_array`] instead.
fn array_element_name(name: &str) -> &str {
    name.strip_suffix("[]").unwrap_or(name)
}

/// `namespace:Script` is not something this lint resolves. A colon name is
/// only treated as a missing `Script:Struct` when the left-hand side is a
/// script (or type) that can already be located.
fn is_unsupported_namespace(external: &mut dyn ExternalSignatures, name: &str) -> bool {
    let Some((owner, member)) = name.rsplit_once(':') else {
        return false;
    };
    if owner.is_empty() || member.is_empty() {
        return false;
    }
    !external.script_exists(owner) && !external.type_exists(owner)
}

fn missing(line: usize, col: usize, name: &str) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!("[warning] Script '{name}' could not be located"),
        rule: RULE,
    }
}

#[cfg(test)]
#[path = "unresolved_script_tests.rs"]
mod tests;

//! Flags a `Native` function/event declared on a linted script whose
//! (script, function) pair isn't one of the base-game native functions
//! listed in `shared/rules/data/native-methods.yaml`.
//!
//! A `Native` declaration has no body of its own — its implementation is
//! supplied by the engine (or, for a modder-authored header script, by an
//! SKSE/F4SE plugin DLL). Since `shared/rules/data/native-methods.yaml` only lists the
//! functions Skyrim's own base-game scripts declare `Native`, a `Native`
//! declaration that doesn't match an entry there is a strong signal the
//! project depends on a native extension rather than anything the base game
//! ships. Disabled by default, since plenty of mods intentionally depend on
//! SKSE/F4SE or another native extension and don't need to be warned about
//! it.
//!
//! Rules are compiled into the `NATIVE_METHODS` array below by `build.rs`
//! at build time, so this never parses YAML at runtime. Unlike most of the
//! other lints in this crate, this works from the parsed AST rather than
//! raw tokens, since it needs each function's declared `Native` flag; a
//! script that doesn't parse cleanly is left unchecked rather than guessed
//! at.

use papyrus_parser::ast::{FunctionDecl, Script};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

pub struct NativeMethodRule {
    pub object: &'static str,
    pub function: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/native_methods_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "native-function-usage";

#[derive(Default)]
struct Collect {
    store: Store,
    script_name: String,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.script_name = script.name.clone();
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if !function.is_native {
            return;
        }
        if is_base_game_native(&self.script_name, &function.name) {
            return;
        }
        self.store.emit(
            function.line,
            1,
            format!(
                "[warning] Native function '{}.{}' isn't supplied by the base game; it likely requires SKSE/F4SE or another native extension",
                self.script_name, function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Native` functions/events not supplied by the base
/// game, per `NATIVE_METHODS`.
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

/// Whether `(script_name, function_name)` matches a base-game native
/// function listed in `shared/rules/data/native-methods.yaml`, case-insensitively
/// (Papyrus identifiers are case-insensitive).
fn is_base_game_native(script_name: &str, function_name: &str) -> bool {
    NATIVE_METHODS.iter().any(|rule| {
        rule.object.eq_ignore_ascii_case(script_name)
            && rule.function.eq_ignore_ascii_case(function_name)
    })
}

#[cfg(test)]
#[path = "native_function_usage_tests.rs"]
mod tests;

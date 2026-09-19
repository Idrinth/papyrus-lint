//! Flags a function or event declared inside a `State` block whose
//! parameter list or return type doesn't match the same-named declaration
//! in the script's "empty state" (i.e. the one declared directly on the
//! script, outside any `State` block).
//!
//! Per the CreationKit wiki's [State
//! Reference](https://ck.uesp.net/wiki/State_Reference): "Every function
//! implemented in a state must also be implemented (with an identical
//! name, return type, and parameter list) in the empty state in either the
//! current script or a parent." A state function whose signature drifts
//! from that empty-state version isn't recognized as an override of it at
//! all, so it silently becomes a distinct, effectively unreachable
//! function instead of the behavior swap the author presumably intended.
//!
//! This only compares against an empty-state declaration already present
//! on the script being linted. Per the quote above, a state function may
//! instead match one declared on a *parent* script, which this lint has no
//! way to resolve (unlike e.g. [`crate::function_override`], there's no
//! `ExternalSignatures` lookup for "the parent's empty-state declaration of
//! this exact name"), so a state function with no local empty-state
//! counterpart is left unflagged rather than guessed at.

use std::collections::HashMap;

use papyrus_parser::ast::{FunctionDecl, Script, TypeName};

use crate::argument_types::format_type;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "state-function-signature";

struct EmptySignature {
    params: Vec<TypeName>,
    return_type: Option<TypeName>,
}

#[derive(Default)]
struct Collect {
    store: Store,
    empty: HashMap<String, EmptySignature>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        let mut empty = HashMap::new();
        for function in &script.functions {
            empty.entry(function.name.to_ascii_lowercase()).or_insert_with(|| {
                EmptySignature {
                    params: function
                        .params
                        .iter()
                        .map(|param| param.type_name.clone())
                        .collect(),
                    return_type: function.return_type.clone(),
                }
            });
        }
        self.empty = empty;
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        let Some(state_name) = &function.state else {
            return;
        };
        let Some(base) = self.empty.get(&function.name.to_ascii_lowercase()) else {
            return;
        };
        check_function(function, base, state_name, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for state-declared functions/events whose parameter
/// list or return type doesn't match the same-named declaration in the
/// script's empty state.
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

fn check_function(
    state_fn: &FunctionDecl,
    base_fn: &EmptySignature,
    state_name: &str,
    store: &mut Store,
) {
    if state_fn.params.len() != base_fn.params.len() {
        store.emit(
            state_fn.line,
            1,
            format!(
                "[error] Function '{}' in state '{}' declares {} but the empty state's declaration declares {}",
                state_fn.name,
                state_name,
                param_count(state_fn.params.len()),
                param_count(base_fn.params.len()),
            ),
            RULE,
        );
    } else {
        for (index, (state_param, base_type)) in
            state_fn.params.iter().zip(&base_fn.params).enumerate()
        {
            if !type_names_match(&state_param.type_name, base_type) {
                store.emit(
                    state_fn.line,
                    1,
                    format!(
                        "[error] Parameter {} of '{}' in state '{}' is declared {} but the empty state's declaration declares {}",
                        index + 1,
                        state_fn.name,
                        state_name,
                        format_type(&state_param.type_name),
                        format_type(base_type),
                    ),
                    RULE,
                );
            }
        }
    }

    if !return_types_match(&state_fn.return_type, &base_fn.return_type) {
        store.emit(
            state_fn.line,
            1,
            format!(
                "[error] Function '{}' in state '{}' declares return type {} but the empty state's declaration declares {}",
                state_fn.name,
                state_name,
                format_return_type(&state_fn.return_type),
                format_return_type(&base_fn.return_type),
            ),
            RULE,
        );
    }
}

/// Papyrus type names are case-insensitive, so `Bool` and `bool` name the
/// same type even though they'd otherwise fail a derived `PartialEq` on
/// [`TypeName`], which compares `name` byte-for-byte.
fn type_names_match(a: &TypeName, b: &TypeName) -> bool {
    a.is_array == b.is_array && a.name.eq_ignore_ascii_case(&b.name)
}

fn return_types_match(a: &Option<TypeName>, b: &Option<TypeName>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => type_names_match(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn param_count(count: usize) -> String {
    if count == 1 {
        "1 parameter".to_string()
    } else {
        format!("{count} parameters")
    }
}

fn format_return_type(return_type: &Option<papyrus_parser::ast::TypeName>) -> String {
    match return_type {
        Some(type_name) => format_type(type_name),
        None => "no value".to_string(),
    }
}

#[cfg(test)]
#[path = "state_function_signature_tests.rs"]
mod tests;

//! Flags implicit Float-to-Int narrowing: a `Float` value declared,
//! assigned, returned, or passed as an argument into an `Int`-typed slot
//! without an explicit `as Int` cast.
//!
//! Unlike the other lints in this crate, this one needs to know a value's
//! inferred type, so it works on the parsed AST (see
//! `papyrus_parser::types`) rather than raw tokens. Scripts that fail to
//! parse simply aren't checked, the same way a lexer failure short-circuits
//! the token-based lints.

use papyrus_parser::ast::{Expr, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::type_flow::{Slot, TypeFlowLint};
use crate::visitor::{LintVisitor, Store};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "float-to-int";

#[derive(Default)]
struct FloatToInt;

impl TypeFlowLint for FloatToInt {
    fn check(
        &mut self,
        target_type: &TypeName,
        value: &Expr,
        env: &TypeEnv,
        slot: Slot<'_>,
        line: usize,
        store: &mut Store,
    ) {
        if !narrows_to_int(target_type, value, env) {
            return;
        }
        let message = match slot {
            Slot::Declaration { name } => format!(
                "[warning] Float value assigned to Int variable '{name}' without an explicit 'as Int' cast"
            ),
            Slot::Assignment { target } => format!(
                "[warning] Float value assigned to Int {} without an explicit 'as Int' cast",
                describe_target(target)
            ),
            Slot::Return { function } => format!(
                "[warning] Float value returned from Int function '{function}' without an explicit 'as Int' cast"
            ),
            Slot::Argument { parameter, function } => format!(
                "[warning] Float value passed as Int parameter '{parameter}' of function '{function}' without an explicit 'as Int' cast"
            ),
        };
        store.emit(line, 1, message, RULE);
    }
}

pub fn visitor() -> LintVisitor {
    crate::type_flow::visitor::<FloatToInt>()
}

/// Checks `source` for Float values narrowed into an Int without an
/// explicit cast.
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

fn is_int(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("int")
}

fn is_float(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("float")
}

/// True when `value` is a Float being narrowed into an Int-typed `target_type`.
///
/// A value's inferred type already reflects any explicit cast it carries
/// (`someFloat as Int` infers as `Int`), so comparing the plain inferred
/// type against the target is enough to let explicit casts through.
fn narrows_to_int(target_type: &TypeName, value: &Expr, env: &TypeEnv) -> bool {
    is_int(target_type) && infer_type(value, env).is_some_and(|value_type| is_float(&value_type))
}

fn describe_target(target: &Expr) -> String {
    match target {
        Expr::Identifier(name) => format!("variable '{name}'"),
        Expr::Member { property, .. } => format!("property '{property}'"),
        Expr::Index { .. } => "array element".to_string(),
        _ => "target".to_string(),
    }
}

#[cfg(test)]
#[path = "float_int_conversion_tests.rs"]
mod tests;

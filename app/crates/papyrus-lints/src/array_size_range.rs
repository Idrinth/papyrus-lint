//! Flags, as an `[error]`, a `new <Type>[<N>]` array creation whose literal
//! `N` falls outside the range Papyrus allows for a script-created array
//! (`0` to `128`), since a size outside that range is almost never
//! intended: per the CreationKit wiki's [Arrays
//! (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page, an array
//! created by a script via `New` (or grown with `Add()`) is hard-capped at
//! 128 elements by the engine, and a negative size makes no sense at all.
//! That cap does not apply to an array returned by a native function or to
//! an editor-populated array `Property`, since neither is created this way.
//!
//! This is a deliberately small, standalone check split out from
//! [`crate::array_bounds`]: unlike that lint's flow-sensitive tracking of
//! which local variable currently holds an array of which size, this one
//! only ever looks at a `new <Type>[<N>]` expression's own literal size, so
//! it has no state to track and nothing else can disable or narrow it.

use papyrus_parser::ast::Expr;

use crate::const_eval::eval_const_int;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "array-size-range";

/// The maximum number of elements an array created by a script (via `New`
/// or grown with `Add()`) can hold, per the CreationKit wiki's [Arrays
/// (Papyrus)](https://ck.uesp.net/wiki/Arrays_(Papyrus)) page.
const MAX_NEW_ARRAY_SIZE: i64 = 128;

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::NewArray { size, .. } = expr else {
            return;
        };
        let Some(literal_size) = eval_const_int(size) else {
            return;
        };
        if (0..=MAX_NEW_ARRAY_SIZE).contains(&literal_size) {
            return;
        }
        self.store.emit(
            ctx.line,
            1,
            format!(
                "[error] Array size {literal_size} is outside the range Papyrus \
                 allows (0 to {MAX_NEW_ARRAY_SIZE}) for an array created with `new`"
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks every function/event in `source` for a `new <Type>[<N>]` whose
/// literal `N` falls outside `0..=128`.
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

#[cfg(test)]
#[path = "array_size_range_tests.rs"]
mod tests;

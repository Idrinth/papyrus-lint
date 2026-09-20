//! Flags calls to functions listed in `shared/rules/data/deprecated-functions.yaml`.
//!
//! The data is compiled into `DEPRECATED_FUNCTIONS` by `build.rs`, so the
//! linter does not parse YAML at runtime. Token-based matching also lets the
//! rule run when the source does not produce a complete AST.

use crate::visitor::{LintVisitor, Store, TokenLint, VisitCtx};
use crate::Diagnostic;
use papyrus_parser::token::{Token, TokenKind};

pub struct DeprecatedFunctionRule {
    pub script: &'static str,
    pub function: &'static str,
    #[allow(dead_code)]
    pub replacement: Option<&'static str>,
    pub level: &'static str,
    pub message: &'static str,
    /// Whether `script` is a native singleton that must be called through
    /// its literal script name rather than through an object instance.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/deprecated_functions_data.rs"));

pub const RULE: &str = "deprecated-functions";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl TokenLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_token(
        &mut self,
        token: &Token,
        index: usize,
        tokens: &[Token],
        _ctx: &mut VisitCtx<'_>,
    ) {
        let TokenKind::Identifier(name) = &token.kind else {
            return;
        };
        if !matches!(tokens.get(index + 1).map(|token| &token.kind), Some(TokenKind::LParen)) {
            return;
        }
        let Some(rule) = find_rule(name) else {
            return;
        };
        if rule.global && !qualifier_matches(tokens, index, rule.script) {
            return;
        }
        self.store.emit(
            token.line,
            token.col,
            format!(
                "[{}] {}.{}: {}",
                rule.level, rule.script, rule.function, rule.message
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Tokens(Box::new(Collect::default()))
}

#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn qualifier_matches(tokens: &[Token], call_index: usize, script: &str) -> bool {
    if call_index < 2 || !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(script)
}

fn find_rule(name: &str) -> Option<&'static DeprecatedFunctionRule> {
    DEPRECATED_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
#[path = "deprecated_functions_tests.rs"]
mod tests;

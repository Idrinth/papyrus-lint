//! Flags a parameter or local/script variable whose name does not follow
//! the Hungarian type-prefix style selected by [`Hungarian`].
//!
//! Disabled by default (see [`crate::config::Rules::hungarian_prefix`]):
//! teams that want Creation Kit prefixes opt in and leave `hungarian` at
//! `allow`; teams that want those prefixes gone opt in and set `forbid`.
//!
//! `identifier-casing` still only checks letter case. A required prefix is
//! lowercase, so `allow` usually wants `identifier_casing: camelCase` (or
//! that rule turned off). This rule does not rename anything: adding or
//! removing the prefix changes the identifier callers see.

use papyrus_parser::ast::{FunctionDecl, Param, TypeName, VariableDecl};
use serde::{Deserialize, Serialize};

use crate::fragment_code;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "hungarian-prefix";

/// Whether [`RULE`] requires Hungarian type prefixes or flags them.
///
/// Ignored while `rules.hungarian_prefix` is off (the default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Hungarian {
    /// Parameters and local/script variables must use the type prefix.
    #[default]
    Allow,
    /// Parameters and local/script variables must not use a type prefix.
    /// `Event` parameters are exempt: stock signatures cannot be renamed.
    Forbid,
}

/// Longer argument prefixes before the single-letter type prefixes, so
/// `aiCount` is argument-integer rather than a local `i` with a leading `a`.
const PREFIXES: &[&str] = &["ab", "ai", "af", "as", "ak", "b", "i", "f", "s", "k"];

#[derive(Default)]
struct Collect {
    store: Store,
    protected: Vec<bool>,
    in_event: bool,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.protected = fragment_code::protected_lines(ctx.source);
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        self.in_event = function.is_event;
    }

    fn visit_variable(&mut self, variable: &VariableDecl, ctx: &mut VisitCtx<'_>) {
        check_decl(
            Decl {
                name: &variable.name,
                kind: "Variable",
                type_name: &variable.type_name,
                is_argument: false,
                line: variable.line,
            },
            ctx.config.hungarian,
            &self.protected,
            &mut self.store,
        );
    }

    fn visit_param(&mut self, param: &Param, ctx: &mut VisitCtx<'_>) {
        if self.in_event && ctx.config.hungarian == Hungarian::Forbid {
            return;
        }
        check_decl(
            Decl {
                name: &param.name,
                kind: "Parameter",
                type_name: &param.type_name,
                is_argument: true,
                line: ctx.line,
            },
            ctx.config.hungarian,
            &self.protected,
            &mut self.store,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for parameters and local/script variables that disagree
/// with [`Hungarian`]. Flagged as an `[info]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged.
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

fn type_letter(type_name: &TypeName) -> &'static str {
    if type_name.name.eq_ignore_ascii_case("bool") {
        "b"
    } else if type_name.name.eq_ignore_ascii_case("int") {
        "i"
    } else if type_name.name.eq_ignore_ascii_case("float") {
        "f"
    } else if type_name.name.eq_ignore_ascii_case("string") {
        "s"
    } else {
        "k"
    }
}

fn expected_prefix(type_name: &TypeName, is_argument: bool) -> &'static str {
    match (is_argument, type_letter(type_name)) {
        (true, "b") => "ab",
        (true, "i") => "ai",
        (true, "f") => "af",
        (true, "s") => "as",
        (true, _) => "ak",
        (false, "b") => "b",
        (false, "i") => "i",
        (false, "f") => "f",
        (false, "s") => "s",
        (false, _) => "k",
    }
}

/// The Hungarian prefix `name` uses, if the leading letters are a known
/// prefix and the rest is empty, a digit, or an uppercase letter.
fn hungarian_prefix(name: &str) -> Option<&'static str> {
    PREFIXES.iter().copied().find(|prefix| {
        name.strip_prefix(prefix).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
    })
}

fn type_label(type_name: &TypeName) -> String {
    if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name.clone()
    }
}

struct Decl<'a> {
    name: &'a str,
    kind: &'a str,
    type_name: &'a TypeName,
    is_argument: bool,
    line: usize,
}

fn check_decl(decl: Decl<'_>, mode: Hungarian, protected: &[bool], store: &mut Store) {
    if protected.get(decl.line).copied().unwrap_or(false) {
        return;
    }
    let expected = expected_prefix(decl.type_name, decl.is_argument);
    let actual = hungarian_prefix(decl.name);
    match mode {
        Hungarian::Allow => {
            if actual == Some(expected) {
                return;
            }
            let label = type_label(decl.type_name);
            let role = if decl.is_argument { "argument " } else { "" };
            let message = match actual {
                Some(found) => format!(
                    "[info] {kind} '{name}' should use Hungarian prefix '{expected}' for {role}{label}, not '{found}'",
                    kind = decl.kind,
                    name = decl.name,
                ),
                None => format!(
                    "[info] {kind} '{name}' should use Hungarian prefix '{expected}' for {role}{label}",
                    kind = decl.kind,
                    name = decl.name,
                ),
            };
            store.emit(decl.line, 1, message, RULE);
        }
        Hungarian::Forbid => {
            let Some(found) = actual else {
                return;
            };
            store.emit(
                decl.line,
                1,
                format!(
                    "[info] {kind} '{name}' uses Hungarian prefix '{found}'",
                    kind = decl.kind,
                    name = decl.name,
                ),
                RULE,
            );
        }
    }
}

#[cfg(test)]
#[path = "hungarian_prefix_tests.rs"]
mod tests;

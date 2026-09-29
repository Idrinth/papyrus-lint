//! Flags a repeated `Import` of the same script (case-insensitive).
//!
//! Papyrus treats identifiers case-insensitively, so `Import Utility` and
//! `Import utility` are the same dependency listed twice. That is clutter
//! and can confuse readers about intended dependencies.
//!
//! Distinct from [`crate::unused_import`]: that rule flags an import whose
//! `Global` functions are never called unqualified (and needs project
//! context to resolve them). This rule flags a used import that is simply
//! listed more than once — including when every occurrence is referenced.
//!
//! Automatically fixable: the fix drops later duplicate `Import` lines and
//! keeps the first.

use std::collections::HashMap;

use papyrus_parser::ast::ImportDecl;

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "duplicate-import";

#[derive(Default)]
struct Collect {
    store: Store,
    /// Lowercased import name → line of the first occurrence kept.
    seen: HashMap<String, usize>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_import(&mut self, import: &ImportDecl, _ctx: &mut VisitCtx<'_>) {
        let key = import.name.to_ascii_lowercase();
        use std::collections::hash_map::Entry;
        match self.seen.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(import.line);
            }
            Entry::Occupied(_) => {
                self.store.emit(
                    import.line,
                    1,
                    format!(
                        "[warning] Import '{}' is duplicated: the same script was already imported earlier",
                        import.name
                    ),
                    RULE,
                );
            }
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for repeated `Import` statements of the same script
/// (compared case-insensitively). Flagged as a `[warning]`.
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

/// Removes every later duplicate `Import` line [`check`] would flag,
/// keeping the first occurrence of each script name. Deletes each whole
/// line (including its line ending) rather than leaving a blank one.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let lines_to_remove = duplicate_import_lines(&script.imports);
    if lines_to_remove.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    let mut line_number = 1usize;
    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        if !lines_to_remove.contains(&line_number) {
            result.push_str(line_and_ending);
        }
        rest = remainder;
        line_number += 1;
    }
    result
}

fn duplicate_import_lines(imports: &[ImportDecl]) -> std::collections::HashSet<usize> {
    use std::collections::hash_map::Entry;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut duplicates = std::collections::HashSet::new();
    for import in imports {
        let key = import.name.to_ascii_lowercase();
        match seen.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(import.line);
            }
            Entry::Occupied(_) => {
                duplicates.insert(import.line);
            }
        }
    }
    duplicates
}

#[cfg(test)]
#[path = "duplicate_import_tests.rs"]
mod tests;

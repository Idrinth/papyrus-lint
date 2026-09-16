//! Flags a `Property` whose declared type, followed through that script's
//! own `Property` declarations across the project, eventually leads back to
//! the script being linted — a circular dependency between two (or more)
//! scripts, e.g.:
//!
//! ```papyrus
//! ScriptName A
//!
//! B Property Little Auto
//! ```
//!
//! together with:
//!
//! ```papyrus
//! ScriptName B
//!
//! A Property Large Auto
//! ```
//!
//! Papyrus itself compiles this fine — a `Property` is just a reference, not
//! an `Extends` chain — but a cycle like this still makes the two (or more)
//! scripts hard to reason about or reuse independently, since neither can be
//! fully understood without the other. A property whose own declared type is
//! the script it's declared on (a direct self-reference, e.g. a linked-list
//! node holding a `Property` of its own type) is never flagged: that's a
//! single script depending on itself, not a dependency between two scripts,
//! and is a common, deliberate pattern rather than a design smell. Since
//! this crate has no filesystem access on its own, following a property's
//! type chain across other scripts needs a resolver that can (e.g. the
//! desktop app's `FunctionTable`); see [`check_with`].

use std::collections::HashSet;

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "circular-dependency";

/// Checks `source` for a `Property` whose declared type, followed through
/// other scripts' own `Property` declarations, cycles back to this script.
/// Since this crate has no filesystem access on its own, no such chain can
/// ever be confirmed this way; see [`check_with`] to actually follow
/// property types across scripts.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but follows each property's declared type through
/// `external`, flagging one whose chain of `Property` declarations across
/// other scripts leads back to this script.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let origin = &script.name;

    let mut diagnostics = Vec::new();
    for property in &script.properties {
        if property.type_name.name.eq_ignore_ascii_case(origin) {
            // A direct self-reference is a script depending on itself, not
            // a cycle between scripts.
            continue;
        }
        let mut visited = HashSet::new();
        if let Some(chain) = cycle_through(&property.type_name.name, origin, external, &mut visited)
        {
            diagnostics.push(Diagnostic {
                line: property.line,
                column: 1,
                message: format!(
                    "[warning] Property '{}' creates a circular dependency: {} -> {}",
                    property.name,
                    origin,
                    chain.join(" -> "),
                ),
                rule: RULE,
            });
        }
    }
    diagnostics
}

/// Depth-first search over the property-type graph starting at
/// `current_type`, looking for a path back to `origin` (matched
/// case-insensitively). `visited` remembers every type already fully
/// explored — whether or not it led back to `origin` — so a cycle among
/// *other* scripts that never reaches `origin` is only ever walked once
/// instead of looping forever. Returns the chain of script names from
/// `current_type` back to `origin` (inclusive of both ends) the first time
/// one is found.
fn cycle_through<E: ExternalSignatures>(
    current_type: &str,
    origin: &str,
    external: &mut E,
    visited: &mut HashSet<String>,
) -> Option<Vec<String>> {
    if current_type.eq_ignore_ascii_case(origin) {
        return Some(vec![current_type.to_string()]);
    }
    if !visited.insert(current_type.to_ascii_lowercase()) {
        return None;
    }
    for next_type in external.property_types(current_type) {
        if let Some(mut chain) = cycle_through(&next_type, origin, external, visited) {
            chain.insert(0, current_type.to_string());
            return Some(chain);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn does_not_flag_anything_without_a_resolver() {
        let diagnostics = check("ScriptName A\n\nB Property Little Auto\n");

        assert!(diagnostics.is_empty());
    }

    struct FakeExternal {
        properties: HashMap<String, Vec<String>>,
    }

    impl FakeExternal {
        fn new(properties: &[(&str, &[&str])]) -> Self {
            FakeExternal {
                properties: properties
                    .iter()
                    .map(|(name, types)| {
                        (
                            name.to_ascii_lowercase(),
                            types.iter().map(|t| t.to_string()).collect(),
                        )
                    })
                    .collect(),
            }
        }
    }

    impl ExternalSignatures for FakeExternal {
        fn lookup(
            &mut self,
            _type_name: &str,
            _function_name: &str,
        ) -> Option<Vec<crate::argument_types::ParamInfo>> {
            None
        }

        fn property_types(&mut self, type_name: &str) -> Vec<String> {
            self.properties
                .get(&type_name.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        }
    }

    #[test]
    fn flags_a_two_script_circular_dependency() {
        let mut external = FakeExternal::new(&[("B", &["A"])]);

        let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0]
            .message
            .contains("circular dependency: A -> B -> A"));
    }

    #[test]
    fn flags_a_three_script_circular_dependency() {
        let mut external = FakeExternal::new(&[("B", &["C"]), ("C", &["A"])]);

        let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("circular dependency: A -> B -> C -> A"));
    }

    #[test]
    fn does_not_flag_a_one_directional_dependency() {
        let mut external = FakeExternal::new(&[("B", &["C"])]);

        let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_direct_self_reference() {
        let mut external = FakeExternal::new(&[("A", &["A"])]);

        let diagnostics = check_with("ScriptName A\n\nA Property Next Auto\n", &mut external);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_infinite_loop_on_a_cycle_that_never_reaches_the_origin() {
        let mut external = FakeExternal::new(&[("B", &["C"]), ("C", &["B"])]);

        let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn matches_script_names_case_insensitively() {
        let mut external = FakeExternal::new(&[("b", &["a"])]);

        let diagnostics = check_with("ScriptName A\n\nB Property Little Auto\n", &mut external);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName A\n\nB Property (\n");
        assert!(diagnostics.is_empty());
    }
}

//! Flags `Bool`/`Int`/`Float`/`String` `Auto`/`AutoReadOnly` properties
//! declared with no explicit default value (e.g. `Int Property Count
//! Auto` rather than `Int Property Count = 0 Auto`), since a property left
//! without one silently falls back to Papyrus's own implicit per-type
//! default (`False`, `0`, `0.0`, or `""`) instead of a value the author
//! actually chose.
//!
//! Disabled by default (see
//! [`crate::config::Rules::default_property_value`]): many existing
//! scripts already rely on Papyrus's implicit defaults for some or all of
//! their properties and don't need every one of them spelling out a value
//! explicitly, so a project has to opt in.
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a property declaration (and its type) apart from other
//! identifiers; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. Object-typed properties, array-typed
//! properties, and full (non-`Auto`/`AutoReadOnly`) properties are never
//! flagged: only a `Bool`, `Int`, `Float`, or `String` scalar property
//! accepts a simple literal default at all.

use papyrus_parser::ast::PropertyDecl;

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "default-property-value";

/// Checks `source` for `Bool`/`Int`/`Float`/`String` `Auto`/`AutoReadOnly`
/// properties with no explicit default value, flagged as a `[warning]`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    script
        .properties
        .iter()
        .filter(|property| (property.is_auto || property.is_auto_read_only) && property.value.is_none())
        .filter_map(|property| {
            let literal = default_literal_for(property)?;
            Some(Diagnostic {
                line: property.line,
                column: 1,
                message: format!(
                    "[warning] {} Property '{}' has no explicit default value; consider '= {}'",
                    property.type_name.name, property.name, literal
                ),
                rule: RULE,
            })
        })
        .collect()
}

/// The literal Papyrus would otherwise use as `property`'s implicit
/// default, or `None` when `property` isn't a scalar `Bool`/`Int`/`Float`/
/// `String` property (an array, or any other type) this lint doesn't cover.
fn default_literal_for(property: &PropertyDecl) -> Option<&'static str> {
    if property.type_name.is_array {
        return None;
    }
    let name = &property.type_name.name;
    if name.eq_ignore_ascii_case("Bool") {
        Some("False")
    } else if name.eq_ignore_ascii_case("Int") {
        Some("0")
    } else if name.eq_ignore_ascii_case("Float") {
        Some("0.0")
    } else if name.eq_ignore_ascii_case("String") {
        Some("\"\"")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_auto_scalar_properties_with_no_default_value() {
        let source = "ScriptName Example\n\nBool Property IsActive Auto\nInt Property Count Auto\nFloat Property Scale Auto\nString Property Label Auto\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 4);
        assert!(diagnostics.iter().all(|d| d.rule == RULE));
        assert!(diagnostics.iter().any(|d| d.message.contains("IsActive") && d.message.contains("False")));
        assert!(diagnostics.iter().any(|d| d.message.contains("Count") && d.message.contains("0")));
        assert!(diagnostics.iter().any(|d| d.message.contains("Scale") && d.message.contains("0.0")));
        assert!(diagnostics.iter().any(|d| d.message.contains("Label") && d.message.contains("\"\"")));
    }

    #[test]
    fn does_not_flag_properties_with_an_explicit_default_value() {
        let source = "ScriptName Example\n\nString Property abc = \"aBc\" Auto\nInt Property Count = 1 Auto\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn flags_auto_read_only_properties_too() {
        let source = "ScriptName Example\n\nInt Property Count AutoReadOnly\n";
        let diagnostics = check(source);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
    }

    #[test]
    fn does_not_flag_object_typed_properties() {
        let source = "ScriptName Example\n\nActor Property PlayerRef Auto\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_array_typed_properties() {
        let source = "ScriptName Example\n\nInt[] Property Counts Auto\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_flag_a_full_non_auto_property() {
        let source = "ScriptName Example\n\nInt Property Count\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n";
        assert!(check(source).is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nInt Property Count = \"unterminated\n").is_empty());
    }
}

//! Metadata tags for every rule in [`crate::KNOWN_RULE_IDS`]: the kind(s) of
//! fix its findings represent (e.g. `"style"`, `"performance"`,
//! `"correctness"`, `"maintainability"`), how important addressing them is
//! to keeping a codebase maintainable, and whether they're auto-fixable.
//! This module only exposes that metadata — a later group-based filtering
//! feature (e.g. "show me only performance findings", or "only auto-fixable
//! ones") is expected to build on it, not implemented here.

use crate::FIXABLE_RULE_IDS;

/// How important fixing a rule's findings is to keeping a codebase
/// maintainable over time. Independent of the `[error]`/`[warning]`/`[info]`
/// level(s) a rule's own diagnostics carry, which instead reflect runtime
/// risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Importance {
    Low,
    Medium,
    High,
}

/// Tags describing one rule, keyed by its [`Diagnostic::rule`](crate::Diagnostic::rule) id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleTags {
    pub rule: &'static str,
    /// Keyword(s) describing the kind(s) of fix this rule's findings
    /// represent. Never empty.
    pub kinds: &'static [&'static str],
    pub importance: Importance,
}

impl RuleTags {
    /// Whether this rule has an automatic fix. Derived from
    /// [`crate::FIXABLE_RULE_IDS`] rather than stored on each entry here, so
    /// the two can never drift out of sync.
    pub fn auto_fixable(&self) -> bool {
        FIXABLE_RULE_IDS.contains(&self.rule)
    }
}

/// Looks up a rule's [`RuleTags`] by its id, matched case-insensitively (the
/// same convention `@disable` directives are matched against — see
/// `disable_comments`). Returns `None` for an id not in
/// [`crate::KNOWN_RULE_IDS`].
pub fn tags_for(rule: &str) -> Option<&'static RuleTags> {
    RULE_TAGS
        .iter()
        .find(|tags| tags.rule.eq_ignore_ascii_case(rule))
}

/// One entry per id in [`crate::KNOWN_RULE_IDS`], in the same order.
pub const RULE_TAGS: &[RuleTags] = &[
    RuleTags {
        rule: crate::trailing_whitespace::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::comma_spacing::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::forbidden_functions::RULE,
        kinds: &["performance", "correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::formid_hex_notation::RULE,
        kinds: &["correctness", "style"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::slow_functions::RULE,
        kinds: &["performance"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_getter::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_property::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::semicolon::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::float_int_conversion::RULE,
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::strict_boolean::RULE,
        kinds: &["correctness", "style"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::argument_types::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::return_types::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::function_override::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::argument_naming::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::state_function_signature::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::numeric_comparison::RULE,
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::indentation::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::cyclomatic_complexity::RULE,
        kinds: &["maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unreachable_statement::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::static_condition::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::division_by_zero::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::empty_body::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_local_variable::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::none_form_usage::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::local_variable_shadowing::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::parameter_reassignment::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::chain_whitespace::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::exclamation_spacing::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::identifier_casing::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::type_casing::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::named_arguments::RULE,
        kinds: &["style", "maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::operator_spacing::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::property_sorting::RULE,
        kinds: &["style"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::explicit_return::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::unchecked_form_parameter::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::unchecked_cast::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::useless_downcast::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::unresolved_script::RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::short_wait_interval::RULE,
        kinds: &["performance"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::goto_state::RULE,
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::state_count::TOO_MANY_STATES_RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: crate::state_count::MULTIPLE_AUTO_STATES_RULE,
        kinds: &["correctness"],
        importance: Importance::High,
    },
    RuleTags {
        rule: "conflicting-script-versions",
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::unused_disable::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::magic_numbers::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::variable_used_before_assignment::RULE,
        kinds: &["correctness"],
        importance: Importance::Medium,
    },
    RuleTags {
        rule: crate::native_function_usage::RULE,
        kinds: &["maintainability"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::repeated_getvalue::RULE,
        kinds: &["performance"],
        importance: Importance::Low,
    },
    RuleTags {
        rule: crate::global_variable_setvalue::RULE,
        kinds: &["correctness", "maintainability"],
        importance: Importance::Medium,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KNOWN_RULE_IDS;
    use std::collections::HashSet;

    #[test]
    fn every_known_rule_id_has_tags() {
        for rule in KNOWN_RULE_IDS {
            assert!(
                tags_for(rule).is_some(),
                "{rule:?} is in KNOWN_RULE_IDS but has no RULE_TAGS entry"
            );
        }
    }

    #[test]
    fn every_tagged_rule_is_a_known_rule_id() {
        for tags in RULE_TAGS {
            assert!(
                KNOWN_RULE_IDS.contains(&tags.rule),
                "{:?} has a RULE_TAGS entry but isn't in KNOWN_RULE_IDS",
                tags.rule
            );
        }
    }

    #[test]
    fn rule_tags_has_no_duplicate_rule_ids() {
        let mut seen = HashSet::new();
        for tags in RULE_TAGS {
            assert!(
                seen.insert(tags.rule),
                "{:?} appears more than once in RULE_TAGS",
                tags.rule
            );
        }
    }

    #[test]
    fn every_rule_has_at_least_one_kind() {
        for tags in RULE_TAGS {
            assert!(
                !tags.kinds.is_empty(),
                "{:?} has no kind keywords",
                tags.rule
            );
        }
    }

    #[test]
    fn auto_fixable_matches_fixable_rule_ids() {
        for tags in RULE_TAGS {
            assert_eq!(
                tags.auto_fixable(),
                FIXABLE_RULE_IDS.contains(&tags.rule),
                "{:?}.auto_fixable() disagrees with FIXABLE_RULE_IDS",
                tags.rule
            );
        }
    }

    #[test]
    fn tags_for_matches_case_insensitively() {
        assert_eq!(
            tags_for("TRAILING-WHITESPACE").map(|t| t.rule),
            Some(crate::trailing_whitespace::RULE)
        );
    }

    #[test]
    fn tags_for_returns_none_for_an_unknown_rule() {
        assert!(tags_for("made-up-rule").is_none());
    }
}

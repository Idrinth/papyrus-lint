//! Metadata tags for every rule in [`crate::KNOWN_RULE_IDS`]: the kind(s) of
//! fix its findings represent (e.g. `"style"`, `"performance"`,
//! `"correctness"`, `"maintainability"`), how important addressing them is
//! to keeping a codebase maintainable, whether they're auto-fixable, and a
//! detailed description of the rule. [`RULE_TAGS`] itself is generated at
//! build time by `build.rs` from `docs/rules.json` — edit that file, not
//! this one, to change a rule's tags, importance, or description.
//! `docs/rules.json` is the source of truth for this metadata (see "Docs
//! sync" in AGENTS.md); `docs/nexuspage.bbcode`'s lint tables and the
//! website's `rules.html` are both generated from it too. This module only
//! exposes that metadata; [`crate::repair_filtered_by_tag`] and the CLI's
//! `--tag <kind>` flag are what actually filter lints/fixes down to one
//! kind at a time, built on top of it.

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

/// The papyrus-lint website's own base URL, from which every rule's
/// [`RuleTags::doc_url`] is built.
pub const WEBSITE_URL: &str = "https://papyrus-lint.idrinth.de";

/// Tags describing one rule, keyed by its [`Diagnostic::rule`](crate::Diagnostic::rule) id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleTags {
    pub rule: &'static str,
    /// The rule's detailed description, copied verbatim from its
    /// `definition` field in `docs/rules.json`, so a consumer (e.g. the
    /// desktop app's "Export for AI" document) can surface the same
    /// explanation the website gives a human reader without needing that
    /// documentation on hand.
    pub description: &'static str,
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

    /// The URL of this rule's own row on the project website's searchable
    /// rules reference (`<website>/rules.html#rule-<rule>`), for a consumer
    /// (the desktop app, the CLI's plain-text/JSON/AI output, the VS Code
    /// extension, the SublimeLinter plugin) to link a finding straight to
    /// its documentation instead of just naming the rule.
    pub fn doc_url(&self) -> String {
        format!("{WEBSITE_URL}/rules.html#rule-{}", self.rule)
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

// One entry per id in `KNOWN_RULE_IDS`. Generated at build time by
// `build.rs` from `docs/rules.json`; do not edit this array by hand.
include!(concat!(env!("OUT_DIR"), "/rule_tags_data.rs"));

#[cfg(test)]
#[path = "tags_tests.rs"]
mod tests;

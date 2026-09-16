//! App version and lint-rule tag metadata exposed to the frontend.

/// A JSON-friendly copy of one [`papyrus_lints::tags::RuleTags`] entry, for
/// the frontend to group/filter lint findings by (e.g. "show me only
/// performance findings", or "only auto-fixable ones") and to include the
/// full rule description in the "Export for AI" document (see
/// `formatIssuesForAi` in `app/src/main.ts`).
#[derive(Debug, PartialEq, serde::Serialize)]
pub(crate) struct RuleTagsInfo {
    rule: String,
    description: &'static str,
    kinds: Vec<&'static str>,
    importance: papyrus_lints::tags::Importance,
    auto_fixable: bool,
    /// This rule's own documentation link (`RuleTags::doc_url`), so the
    /// frontend can link a finding straight to its explanation on the
    /// project website instead of just naming the rule.
    doc_url: String,
}

/// Returns every built-in lint rule's tag metadata (see
/// [`papyrus_lints::tags`]), for the frontend to surface alongside each
/// finding and to drive filtering the lint results by kind, importance, or
/// auto-fixability.
#[tauri::command]
pub(crate) fn list_rule_tags() -> Vec<RuleTagsInfo> {
    papyrus_lints::tags::RULE_TAGS
        .iter()
        .map(|tags| RuleTagsInfo {
            rule: tags.rule.to_string(),
            description: tags.description,
            kinds: tags.kinds.to_vec(),
            importance: tags.importance,
            auto_fixable: tags.auto_fixable(),
            doc_url: tags.doc_url(),
        })
        .collect()
}

/// Returns the desktop app's version (from `app/src-tauri/Cargo.toml`, kept in
/// sync with `package.json`/`tauri.conf.json` at release time), so the
/// frontend can display it to the user.
#[tauri::command]
pub(crate) fn get_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_app_version_returns_the_crate_version() {
        assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn list_rule_tags_reports_every_known_rules_metadata() {
        let tags = list_rule_tags();
        assert_eq!(tags.len(), papyrus_lints::tags::RULE_TAGS.len());

        let trailing_whitespace = tags
            .iter()
            .find(|info| info.rule == papyrus_lints::trailing_whitespace::RULE)
            .expect("trailing-whitespace should be tagged");
        assert!(!trailing_whitespace.description.is_empty());
        assert_eq!(trailing_whitespace.kinds, vec!["style"]);
        assert_eq!(
            trailing_whitespace.importance,
            papyrus_lints::tags::Importance::Low
        );
        assert!(trailing_whitespace.auto_fixable);
        assert_eq!(
            trailing_whitespace.doc_url,
            "https://papyrus-lint.idrinth.de/#lint-trailing-whitespace"
        );

        let argument_types = tags
            .iter()
            .find(|info| info.rule == papyrus_lints::argument_types::RULE)
            .expect("argument-types should be tagged");
        assert!(!argument_types.auto_fixable);
    }
}

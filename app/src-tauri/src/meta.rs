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
#[path = "meta_tests.rs"]
mod tests;

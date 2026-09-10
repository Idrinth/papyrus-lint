//! Human-facing metadata for the desktop app's first-run preset picker
//! (see `app/src-tauri/src/lib.rs`'s `list_config_presets`/
//! `apply_config_preset` commands), shown when a project directory has no
//! `papyrus-lint.yaml`/`.yml` of its own yet.
//!
//! The presets themselves — their embedded YAML, name parsing, and the
//! executable-adjacent base-config layering — are [`crate::config::Preset`],
//! shared with the CLI's own `init --preset <name>` flag. This module only
//! adds the label/description text a user picks between; resolving a
//! chosen id back to a config goes straight through
//! [`crate::config::Preset::parse`] and
//! [`crate::config::initialize_default_config`].

use crate::config::PRESET_NAMES;

/// A preset's identity and description, for the frontend to render as a
/// picker.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PresetInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// Label/description text for each of [`PRESET_NAMES`], in the same order.
/// Kept in sync with `docs/presets/*.yaml`'s own header comments, which go
/// into more detail about exactly what each preset turns on/off.
const DESCRIPTIONS: [(&str, &str, &str); 3] = [
    (
        "strict",
        "Strict",
        "Catches everything the linter knows how to catch, including pure \
         style/naming nits and purely informational findings. Best suited to a project \
         written for this linter from day one, or a team willing to triage every finding.",
    ),
    (
        "standard",
        "Standard",
        "A middle ground for most projects: every rule that can catch a real \
         bug or performance problem, plus the free, auto-fixable formatting rules. Naming \
         conventions and purely informational notices are left off.",
    ),
    (
        "careful",
        "Careful",
        "The quietest option: meant for a first pass over a project that wasn't \
         necessarily written with this linter in mind. Only rules that catch real \
         correctness/performance problems are on; formatting and style rules stay off.",
    ),
];

/// Every built-in preset's identity/description, in [`PRESET_NAMES`]'s
/// order.
pub fn all() -> Vec<PresetInfo> {
    PRESET_NAMES
        .iter()
        .map(|&id| {
            let (_, label, description) = DESCRIPTIONS
                .iter()
                .find(|(name, _, _)| *name == id)
                .unwrap_or_else(|| panic!("no description registered for preset {id:?}"));
            PresetInfo {
                id,
                label,
                description,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Preset;

    #[test]
    fn all_lists_every_preset_with_a_non_empty_label_and_description() {
        let presets = all();

        assert_eq!(presets.len(), PRESET_NAMES.len());
        for preset in &presets {
            assert!(!preset.label.is_empty());
            assert!(!preset.description.is_empty());
            assert!(
                Preset::parse(preset.id).is_some(),
                "{:?} should be a name Preset::parse recognizes",
                preset.id
            );
        }
    }

    #[test]
    fn all_matches_preset_names_order() {
        let ids: Vec<&str> = all().into_iter().map(|preset| preset.id).collect();
        assert_eq!(ids, PRESET_NAMES.to_vec());
    }
}

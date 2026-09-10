//! Human-facing metadata for the desktop app's first-run preset picker
//! (see `app/src-tauri/src/lib.rs`'s `list_config_presets`/
//! `apply_config_preset` commands), shown when a project directory has no
//! `papyrus-lint.yaml`/`.yml` of its own yet.
//!
//! The presets themselves — their embedded YAML, name parsing, and the
//! executable-adjacent base-config layering — are [`crate::config::Preset`],
//! shared with the CLI's own `init --preset <name>` flag. This module adds
//! the label/description text a user picks between, for both the three
//! built-in presets and any user preset found under the executable-adjacent
//! `presets` directory (see [`crate::config::user_presets_dir`]); resolving
//! a chosen id back to a config goes straight through
//! [`crate::config::Preset::parse`] and
//! [`crate::config::initialize_default_config`].

use std::path::Path;

use crate::config::{self, PRESET_NAMES};

/// A preset's identity and description, for the frontend to render as a
/// picker. Owned (rather than `&'static str`) since a user preset's id/
/// label/description are derived from a file name discovered at runtime,
/// unlike a built-in preset's.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PresetInfo {
    pub id: String,
    pub label: String,
    pub description: String,
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

/// Every built-in preset's identity/description (in [`PRESET_NAMES`]'s
/// order), followed by every user preset found under the
/// executable-adjacent `presets` directory (see
/// [`crate::config::user_presets_dir`]), in alphabetical order.
pub fn all() -> Vec<PresetInfo> {
    all_with_user_presets_dir(config::user_presets_dir().as_deref())
}

/// Same as [`all`], but takes the user presets directory to scan
/// explicitly, rather than assuming it's next to the running executable.
/// Split out so tests can exercise the user-preset listing without
/// depending on the test binary's own `current_exe()`.
fn all_with_user_presets_dir(dir: Option<&Path>) -> Vec<PresetInfo> {
    let mut all: Vec<PresetInfo> = PRESET_NAMES
        .iter()
        .map(|&id| {
            let (_, label, description) = DESCRIPTIONS
                .iter()
                .find(|(name, _, _)| *name == id)
                .unwrap_or_else(|| panic!("no description registered for preset {id:?}"));
            PresetInfo {
                id: id.to_string(),
                label: label.to_string(),
                description: description.to_string(),
            }
        })
        .collect();

    if let Some(dir) = dir {
        all.extend(config::list_user_preset_names(dir).into_iter().map(|name| PresetInfo {
            id: name.clone(),
            label: name.clone(),
            description: format!(
                "A custom preset loaded from {name}.yaml in the presets directory next to the executable."
            ),
        }));
    }

    all
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::config::Preset;

    #[test]
    fn all_lists_every_built_in_preset_with_a_non_empty_label_and_description() {
        // No `presets` directory exists next to the test binary, so `all()`
        // reports only the three built-ins here.
        let presets = all();

        assert_eq!(presets.len(), PRESET_NAMES.len());
        for preset in &presets {
            assert!(!preset.label.is_empty());
            assert!(!preset.description.is_empty());
            assert!(
                Preset::parse(&preset.id).is_some(),
                "{:?} should be a name Preset::parse recognizes",
                preset.id
            );
        }
    }

    #[test]
    fn all_matches_preset_names_order() {
        let ids: Vec<String> = all().into_iter().map(|preset| preset.id).collect();
        assert_eq!(ids, PRESET_NAMES.to_vec());
    }

    #[test]
    fn preset_info_serializes_with_the_frontend_field_names() {
        let preset = PresetInfo {
            id: "team-rules".to_string(),
            label: "Team rules".to_string(),
            description: "The team's shared lint configuration.".to_string(),
        };

        assert_eq!(
            serde_json::to_value(preset).expect("preset info should serialize"),
            json!({
                "id": "team-rules",
                "label": "Team rules",
                "description": "The team's shared lint configuration.",
            })
        );
    }

    #[test]
    fn all_with_no_user_preset_directory_returns_only_built_ins() {
        let presets = all_with_user_presets_dir(None);

        assert_eq!(presets.len(), PRESET_NAMES.len());
        assert_eq!(
            presets
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            PRESET_NAMES
        );
    }

    #[test]
    fn all_with_user_presets_dir_appends_custom_presets_after_the_built_ins() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(dir.path().join("abc.yaml"), "").expect("failed to write preset file");

        let presets = all_with_user_presets_dir(Some(dir.path()));

        assert_eq!(presets.len(), PRESET_NAMES.len() + 1);
        let custom = presets
            .last()
            .expect("should have a trailing custom preset");
        assert_eq!(custom.id, "abc");
        assert_eq!(custom.label, "abc");
        assert!(!custom.description.is_empty());
    }

    #[test]
    fn all_with_user_presets_dir_keeps_custom_names_sorted_and_describes_their_files() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(dir.path().join("Zebra.yml"), "").expect("failed to write preset file");
        fs::write(dir.path().join("alpha.yaml"), "").expect("failed to write preset file");
        fs::write(dir.path().join("ignored.txt"), "").expect("failed to write non-preset file");

        let custom = &all_with_user_presets_dir(Some(dir.path()))[PRESET_NAMES.len()..];

        assert_eq!(
            custom
                .iter()
                .map(|preset| preset.id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "Zebra"]
        );
        assert_eq!(custom[0].label, "alpha");
        assert!(custom[0].description.contains("alpha.yaml"));
        assert!(custom[1].description.contains("Zebra.yaml"));
    }

    #[test]
    fn all_with_user_presets_dir_ignores_a_missing_directory() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let presets = all_with_user_presets_dir(Some(&dir.path().join("does-not-exist")));

        assert_eq!(presets.len(), PRESET_NAMES.len());
    }
}

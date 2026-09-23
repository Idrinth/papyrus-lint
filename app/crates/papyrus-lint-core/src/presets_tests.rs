use std::fs;

use serde_json::json;

use super::*;
use papyrus_lint_config::presets::Preset;

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

//! Human-facing metadata for the desktop app's first-run preset picker
//! (see `app/src-tauri/src/config_presets.rs`'s `list_config_presets`/
//! `apply_config_preset` commands), shown when a project directory has no
//! `papyrus-lint.yaml`/`.yml` of its own yet.
//!
//! The presets themselves — their embedded YAML, name parsing, and the
//! executable-adjacent base-config layering — are
//! [`papyrus_lint_config::presets::Preset`], shared with the CLI's own
//! `init --preset <name>` flag. This module adds the label/description text
//! a user picks between, for both the three built-in presets and any user
//! preset found under the executable-adjacent `presets` directory (see
//! [`papyrus_lint_config::presets::user_presets_dir`]); resolving a chosen id
//! back to a config goes straight through
//! [`papyrus_lint_config::presets::Preset::parse`] and
//! [`papyrus_lint_config::presets::initialize_default_config`].

use std::path::Path;

use papyrus_lint_config::presets::{self as config, PRESET_NAMES};

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

// Generated from the first descriptive paragraph in each built-in preset's
// YAML header, making those headers the GUI picker's single source of truth.
include!(concat!(env!("OUT_DIR"), "/preset_descriptions.rs"));

/// Every built-in preset's identity/description (in [`PRESET_NAMES`]'s
/// order), followed by every user preset found under the
/// executable-adjacent `presets` directory (see
/// [`papyrus_lint_config::presets::user_presets_dir`]), in alphabetical order.
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
#[path = "presets_tests.rs"]
mod tests;

//! Built-in starting-point configurations ("presets"), embedded at compile
//! time from `docs/presets/*.yaml`, for a project that doesn't have a
//! `papyrus-lint.yaml`/`.yml` of its own yet. The desktop app's frontend
//! offers these (see `app/src-tauri/src/lib.rs`'s `list_config_presets`/
//! `apply_config_preset` commands) as a first-run picker instead of
//! silently linting a brand-new project against the engine's built-in
//! defaults (the "strict" preset below) without asking.
//!
//! Each preset file is a complete `papyrus-lint.yaml` (the same format
//! [`crate::config`] reads from a project directory), so only its
//! [`papyrus_lints::Config`] portion is used here; the non-lint keys it
//! also sets (`compiler_path`, `additional_script_roots`, ...) match this
//! crate's own defaults for those keys anyway, and [`crate::config::save`]
//! (used to write the chosen preset into a project) never touches them.

use papyrus_lints::Config;

/// One built-in preset's identity, embedded YAML, and how the CLI/desktop
/// app should describe it to a user choosing between them.
struct PresetDef {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    yaml: &'static str,
}

const PRESETS: &[PresetDef] = &[
    PresetDef {
        id: "strict",
        label: "Strict",
        description: "Catches everything the linter knows how to catch, including pure \
            style/naming nits and purely informational findings. Best suited to a project \
            written for this linter from day one, or a team willing to triage every finding.",
        yaml: include_str!("../../../../docs/presets/papyrus-lint.strict.yaml"),
    },
    PresetDef {
        id: "standard",
        label: "Standard",
        description: "A middle ground for most projects: every rule that can catch a real \
            bug or performance problem, plus the free, auto-fixable formatting rules. Naming \
            conventions and purely informational notices are left off.",
        yaml: include_str!("../../../../docs/presets/papyrus-lint.standard.yaml"),
    },
    PresetDef {
        id: "careful",
        label: "Careful",
        description: "The quietest option: meant for a first pass over a project that wasn't \
            necessarily written with this linter in mind. Only rules that catch real \
            correctness/performance problems are on; formatting and style rules stay off.",
        yaml: include_str!("../../../../docs/presets/papyrus-lint.careful.yaml"),
    },
];

/// A preset's identity and description, for the frontend to render as a
/// picker without needing the (potentially large) [`Config`] it resolves
/// to until the user actually picks one.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PresetInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// Every built-in preset's identity/description, in the order they should
/// be offered (least to most aggressive doesn't apply here; this is
/// "strict, standard, careful", matching the order the draft files were
/// authored in and cross-reference each other by).
pub fn all() -> Vec<PresetInfo> {
    PRESETS
        .iter()
        .map(|preset| PresetInfo {
            id: preset.id,
            label: preset.label,
            description: preset.description,
        })
        .collect()
}

/// Resolves `id` (case-insensitively) to its embedded [`Config`], or
/// `None` if `id` doesn't name a known preset.
pub fn config_for(id: &str) -> Option<Config> {
    let preset = PRESETS
        .iter()
        .find(|preset| preset.id.eq_ignore_ascii_case(id))?;
    // The embedded YAML is checked into the repository and covered by the
    // tests below, so a parse failure here would mean a corrupted build
    // rather than anything a caller can recover from.
    Some(
        papyrus_lints::config::parse(preset.yaml)
            .unwrap_or_else(|err| panic!("built-in preset {:?} failed to parse: {err}", preset.id)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_preset_with_a_non_empty_label_and_description() {
        let presets = all();

        assert_eq!(presets.len(), 3);
        for preset in &presets {
            assert!(!preset.label.is_empty());
            assert!(!preset.description.is_empty());
        }
    }

    #[test]
    fn config_for_resolves_every_id_listed_by_all() {
        for preset in all() {
            assert!(
                config_for(preset.id).is_some(),
                "config_for should resolve {:?}",
                preset.id
            );
        }
    }

    #[test]
    fn config_for_is_case_insensitive() {
        assert!(config_for("Strict").is_some());
        assert!(config_for("STANDARD").is_some());
    }

    #[test]
    fn config_for_returns_none_for_an_unknown_preset() {
        assert_eq!(config_for("nonexistent"), None);
    }

    #[test]
    fn strict_preset_matches_the_engine_default() {
        // The "strict" preset is documented as identical to
        // Config::default() (see docs/presets/papyrus-lint.strict.yaml);
        // this pins that claim so the two can't silently drift apart.
        assert_eq!(config_for("strict"), Some(Config::default()));
    }

    #[test]
    fn standard_preset_disables_pure_style_advisories() {
        let config = config_for("standard").expect("standard preset should resolve");

        assert!(!config.rules.identifier_casing);
        assert!(!config.rules.type_casing);
        assert!(config.rules.trailing_whitespace);
        assert!(config.rules.argument_types);
    }

    #[test]
    fn careful_preset_relaxes_complexity_thresholds_and_disables_formatting_rules() {
        let config = config_for("careful").expect("careful preset should resolve");

        assert_eq!(config.cyclomatic_complexity_warning, 20);
        assert_eq!(config.cyclomatic_complexity_error, 40);
        assert!(!config.rules.trailing_whitespace);
        assert!(!config.rules.indentation);
        assert!(config.rules.division_by_zero);
        assert!(config.rules.argument_types);
    }
}

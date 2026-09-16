//! Built-in and user configuration preset commands.

use std::path::PathBuf;

use papyrus_lint_config as config;
use papyrus_lint_core::presets;

/// Returns every configuration preset's identity/description — the three
/// built-ins plus any user preset found under a `presets` directory next to
/// the running executable (see [`papyrus_lint_core::presets`]) — for the
/// frontend's first-run picker shown when a project directory has no
/// `papyrus-lint.yaml`/`.yml` yet.
#[tauri::command(async)]
pub(crate) fn list_config_presets() -> Vec<presets::PresetInfo> {
    presets::all()
}

/// Seeds `dir`'s papyrus-lint config file from the named preset (a built-in
/// one, or a user preset found under the executable-adjacent `presets`
/// directory), for the frontend's first-run picker, via the same
/// [`config::initialize_default_config`] the CLI's `init --preset` uses:
/// refuses to replace an existing config file, and still layers in an
/// executable-adjacent base config over the selected preset if one exists.
/// Errors if `preset` doesn't name a known preset.
#[tauri::command(async)]
pub(crate) fn apply_config_preset(dir: String, preset: String) -> Result<(), String> {
    let preset = config::Preset::parse(&preset)
        .ok_or_else(|| format!("unknown configuration preset: {preset}"))?;
    config::initialize_default_config(&PathBuf::from(dir), preset)?;
    Ok(())
}

/// Returns the named preset's (built-in, or user preset found under the
/// executable-adjacent `presets` directory) lint rule/formatting settings
/// only, via [`config::preset_lint_config_default`] — not the
/// project-level `compiler_path`/`additional_script_roots`/`lookup_script_roots`/
/// `compile_check`/`strict_achlist_scope` settings [`apply_config_preset`] also seeds a
/// brand new project's file with. Used by the Settings tab's "Reset to
/// preset" button to overwrite its currently edited settings back to a
/// preset in place, after the user has confirmed discarding whatever's
/// currently configured — unlike `apply_config_preset`, this never touches
/// (or requires the absence of) a project's config file itself, since the
/// frontend persists the returned settings through its own existing save
/// path. Errors if `preset` doesn't name a known preset.
#[tauri::command(async)]
pub(crate) fn get_preset_lint_config(preset: String) -> Result<papyrus_lints::Config, String> {
    let preset = config::Preset::parse(&preset)
        .ok_or_else(|| format!("unknown configuration preset: {preset}"))?;
    config::preset_lint_config_default(preset)
}

/// Saves the desktop app's currently edited lint settings (the Settings
/// tab's own fields, not the project-level `compiler_path`/`additional_script_roots`/
/// `lookup_script_roots`/`compile_check`/`strict_achlist_scope` next to them) as a new user
/// preset named `name`, in the same executable-adjacent `presets`
/// directory [`list_config_presets`]/[`apply_config_preset`] use, so it's
/// immediately selectable from the first-run picker (or the CLI's
/// `--preset <name>`) afterward. Refuses to replace an existing same-named
/// preset (matched case-insensitively) unless `overwrite` is true. Errors
/// if `name` is blank or matches a built-in preset name.
#[tauri::command(async)]
pub(crate) fn save_config_as_preset(
    config: papyrus_lints::Config,
    name: String,
    overwrite: bool,
) -> Result<(), String> {
    config::save_user_preset(&name, &config, overwrite)?;
    Ok(())
}

/// Renames the user preset named `old_name` to `new_name`, in the same
/// executable-adjacent `presets` directory [`list_config_presets`]/
/// [`save_config_as_preset`] use, for the desktop app's preset management
/// tab. Refuses to replace an existing same-named preset (matched
/// case-insensitively) unless `overwrite` is true. Errors if `new_name` is
/// blank, matches a built-in preset name, or `old_name` doesn't name an
/// existing user preset.
#[tauri::command(async)]
pub(crate) fn rename_user_preset(
    old_name: String,
    new_name: String,
    overwrite: bool,
) -> Result<(), String> {
    config::rename_user_preset(&old_name, &new_name, overwrite)?;
    Ok(())
}

/// Deletes the user preset named `name`, from the same executable-adjacent
/// `presets` directory [`list_config_presets`]/[`save_config_as_preset`]
/// use, for the desktop app's preset management tab. Errors if no preset
/// named `name` exists.
#[tauri::command(async)]
pub(crate) fn delete_user_preset(name: String) -> Result<(), String> {
    config::delete_user_preset(&name)
}

/// Returns the raw YAML content of the user preset named `name`, for the
/// desktop app's preset management tab to offer as a download. Errors if
/// no preset named `name` exists.
#[tauri::command(async)]
pub(crate) fn export_user_preset(name: String) -> Result<String, String> {
    config::read_user_preset_yaml(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use crate::lint_config::{load_lint_config, save_lint_config};

    #[test]
    fn list_config_presets_reports_every_built_in_preset() {
        let presets = list_config_presets();

        assert_eq!(presets.len(), 3);
        assert!(presets.iter().any(|preset| preset.id == "strict"));
        assert!(presets.iter().any(|preset| preset.id == "standard"));
        assert!(presets.iter().any(|preset| preset.id == "careful"));
    }

    #[test]
    fn apply_config_preset_seeds_a_projects_config_from_the_named_preset() {
        let dir = tempdir().unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();

        apply_config_preset(dir_string.clone(), "careful".to_string()).unwrap();

        let config = load_lint_config(dir_string).unwrap();
        assert_eq!(config.cyclomatic_complexity_warning, 20);
        assert!(!config.rules.trailing_whitespace);
    }

    #[test]
    fn apply_config_preset_rejects_an_unknown_preset() {
        let dir = tempdir().unwrap();

        let error = apply_config_preset(
            dir.path().to_string_lossy().into_owned(),
            "nonexistent".to_string(),
        )
        .expect_err("should reject an unknown preset");

        assert!(error.contains("nonexistent"));
    }

    #[test]
    fn get_preset_lint_config_resolves_a_built_in_preset() {
        let config = get_preset_lint_config("careful".to_string()).unwrap();

        assert_eq!(config.cyclomatic_complexity_warning, 20);
        assert!(!config.rules.trailing_whitespace);
    }

    #[test]
    fn get_preset_lint_config_rejects_an_unknown_preset() {
        let error = get_preset_lint_config("nonexistent".to_string())
            .expect_err("should reject an unknown preset");

        assert!(error.contains("nonexistent"));
    }

    #[test]
    fn save_config_as_preset_rejects_a_blank_name() {
        let error =
            save_config_as_preset(papyrus_lints::Config::default(), "   ".to_string(), false)
                .expect_err("blank name should be rejected");

        assert!(error.contains("must not be blank"));
    }

    #[test]
    fn save_config_as_preset_rejects_a_built_in_preset_name() {
        let error = save_config_as_preset(
            papyrus_lints::Config::default(),
            "strict".to_string(),
            false,
        )
        .expect_err("built-in preset name should be rejected");

        assert!(error.contains("built-in preset name"));
    }

    #[test]
    fn rename_user_preset_rejects_a_blank_new_name() {
        let error = rename_user_preset("old-name".to_string(), "   ".to_string(), false)
            .expect_err("blank new name should be rejected");

        assert!(error.contains("must not be blank"));
    }

    #[test]
    fn rename_user_preset_rejects_a_built_in_preset_name() {
        let error = rename_user_preset("old-name".to_string(), "strict".to_string(), false)
            .expect_err("built-in preset name should be rejected");

        assert!(error.contains("built-in preset name"));
    }

    #[test]
    fn delete_user_preset_errors_for_an_unknown_preset() {
        let error = delete_user_preset(format!("no-such-preset-{}", std::process::id()))
            .expect_err("deleting an unknown preset should fail");

        assert!(error.contains("no preset named"));
    }

    #[test]
    fn export_user_preset_errors_for_an_unknown_preset() {
        let error = export_user_preset(format!("no-such-preset-{}", std::process::id()))
            .expect_err("exporting an unknown preset should fail");

        assert!(error.contains("unknown preset"));
    }

    #[test]
    fn apply_config_preset_refuses_to_replace_an_existing_config() {
        let dir = tempdir().unwrap();
        let dir_string = dir.path().to_string_lossy().into_owned();
        save_lint_config(dir_string.clone(), papyrus_lints::Config::default()).unwrap();

        let error = apply_config_preset(dir_string, "careful".to_string())
            .expect_err("should refuse to replace an existing config");

        assert!(error.contains("config already exists"));
    }

    #[test]
    fn apply_config_preset_rejects_a_blank_preset_name() {
        let dir = tempdir().unwrap();

        let error =
            apply_config_preset(dir.path().to_string_lossy().into_owned(), "   ".to_string())
                .expect_err("blank preset should be rejected");

        assert!(error.contains("unknown configuration preset"));
    }

    #[test]
    fn get_preset_lint_config_rejects_a_blank_preset_name() {
        let error = get_preset_lint_config(" \t ".to_string())
            .expect_err("blank preset should be rejected");

        assert!(error.contains("unknown configuration preset"));
    }

    #[test]
    fn get_preset_lint_config_resolves_each_built_in_preset_case_insensitively() {
        let strict = get_preset_lint_config("strict".to_string()).unwrap();
        assert!(strict.rules.trailing_whitespace);
        assert!(strict.rules.identifier_casing);

        let standard = get_preset_lint_config("STANDARD".to_string()).unwrap();
        assert!(standard.rules.trailing_whitespace);
        assert!(!standard.rules.identifier_casing);

        let careful = get_preset_lint_config(" Careful ".to_string()).unwrap();
        assert!(!careful.rules.trailing_whitespace);
        assert_eq!(careful.cyclomatic_complexity_warning, 20);
    }
}

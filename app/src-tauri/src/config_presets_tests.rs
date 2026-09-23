use super::*;
use std::sync::Mutex;
use tempfile::tempdir;

use crate::lint_config::{load_lint_config, save_lint_config};

// User presets live beside the test executable, so tests which inspect or
// mutate that shared directory must not run at the same time.
static USER_PRESETS: Mutex<()> = Mutex::new(());

struct UserPresetCleanup(Vec<String>);

impl Drop for UserPresetCleanup {
    fn drop(&mut self) {
        for name in &self.0 {
            let _ = delete_user_preset(name.clone());
        }
    }
}

#[test]
fn list_config_presets_reports_every_built_in_preset() {
    let _guard = USER_PRESETS.lock().unwrap();
    let presets = list_config_presets();

    assert!(presets.len() >= 3);
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
    let error = save_config_as_preset(papyrus_lints::Config::default(), "   ".to_string(), false)
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

    let error = apply_config_preset(dir.path().to_string_lossy().into_owned(), "   ".to_string())
        .expect_err("blank preset should be rejected");

    assert!(error.contains("unknown configuration preset"));
}

#[test]
fn get_preset_lint_config_rejects_a_blank_preset_name() {
    let error =
        get_preset_lint_config(" \t ".to_string()).expect_err("blank preset should be rejected");

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

#[test]
fn user_preset_commands_cover_the_full_management_lifecycle() {
    let _guard = USER_PRESETS.lock().unwrap();
    let suffix = std::process::id();
    let original_name = format!("desktop-test-{suffix}");
    let renamed_name = format!("desktop-renamed-{suffix}");
    let _cleanup = UserPresetCleanup(vec![original_name.clone(), renamed_name.clone()]);
    let mut config = papyrus_lints::Config {
        indentation_width: 2,
        ..Default::default()
    };

    save_config_as_preset(config.clone(), original_name.clone(), false).unwrap();
    let exported = export_user_preset(original_name.clone()).unwrap();
    assert!(exported.contains("indentation_width: 2"));
    assert_eq!(
        get_preset_lint_config(original_name.clone()).unwrap(),
        config
    );
    assert!(list_config_presets()
        .iter()
        .any(|preset| preset.id == original_name));

    let error = save_config_as_preset(config.clone(), original_name.clone(), false)
        .expect_err("saving over a preset should require confirmation");
    assert!(error.contains("already exists"));

    config.indentation_width = 8;
    save_config_as_preset(config.clone(), original_name.clone(), true).unwrap();
    assert_eq!(
        get_preset_lint_config(original_name.clone()).unwrap(),
        config
    );

    rename_user_preset(original_name.clone(), renamed_name.clone(), false).unwrap();
    assert!(export_user_preset(original_name).is_err());
    assert_eq!(
        get_preset_lint_config(renamed_name.clone()).unwrap(),
        config
    );

    delete_user_preset(renamed_name.clone()).unwrap();
    assert!(export_user_preset(renamed_name).is_err());
}

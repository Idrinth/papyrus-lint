//! Built-in and user configuration preset commands.

use std::path::PathBuf;

use papyrus_lint_config::presets as config_presets;
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
/// [`config_presets::initialize_default_config`] the CLI's `init --preset` uses:
/// refuses to replace an existing config file, and still layers in an
/// executable-adjacent base config over the selected preset if one exists.
/// Errors if `preset` doesn't name a known preset.
#[tauri::command(async)]
pub(crate) fn apply_config_preset(dir: String, preset: String) -> Result<(), String> {
    let preset = config_presets::Preset::parse(&preset)
        .ok_or_else(|| format!("unknown configuration preset: {preset}"))?;
    config_presets::initialize_default_config(&PathBuf::from(dir), preset)?;
    Ok(())
}

/// Returns the named preset's (built-in, or user preset found under the
/// executable-adjacent `presets` directory) lint rule/formatting settings
/// only, via [`config_presets::preset_lint_config_default`] — not the
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
    let preset = config_presets::Preset::parse(&preset)
        .ok_or_else(|| format!("unknown configuration preset: {preset}"))?;
    config_presets::preset_lint_config_default(preset)
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
    config_presets::save_user_preset(&name, &config, overwrite)?;
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
    config_presets::rename_user_preset(&old_name, &new_name, overwrite)?;
    Ok(())
}

/// Deletes the user preset named `name`, from the same executable-adjacent
/// `presets` directory [`list_config_presets`]/[`save_config_as_preset`]
/// use, for the desktop app's preset management tab. Errors if no preset
/// named `name` exists.
#[tauri::command(async)]
pub(crate) fn delete_user_preset(name: String) -> Result<(), String> {
    config_presets::delete_user_preset(&name)
}

/// Returns the raw YAML content of the user preset named `name`, for the
/// desktop app's preset management tab to offer as a download. Errors if
/// no preset named `name` exists.
#[tauri::command(async)]
pub(crate) fn export_user_preset(name: String) -> Result<String, String> {
    config_presets::read_user_preset_yaml(&name)
}

#[cfg(test)]
#[path = "config_presets_tests.rs"]
mod tests;

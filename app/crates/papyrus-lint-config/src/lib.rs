//! Project `papyrus-lint.yaml` / `.yml` loading, saving, presets, and
//! app-level settings (compiler path, script roots, compile-check).
//!
//! Produces the [`papyrus_lints::Config`] passed to every check/fix job,
//! plus the project-level settings that live in the same file alongside it.
//!
//! This crate is the only place those concerns live. `papyrus_lint_core`
//! re-exports it as `papyrus_lint_core::config` so existing callers keep
//! working.

mod detect;
mod paths;
mod presets;
mod project_file;
mod settings;

#[cfg(test)]
mod test_support;

pub use detect::{
    auto_detect_compiler_path, detected_skyrim_install_path, detected_skyrim_script_lookup_dirs,
};
pub use paths::config_file_path;
pub use presets::{
    add_user_preset, delete_user_preset, initialize_default_config, list_user_preset_names,
    preset_lint_config, preset_lint_config_default, read_user_preset_yaml, rename_user_preset,
    save_user_preset, user_presets_dir, AddPresetError, Preset, PRESET_NAMES,
    USER_PRESETS_DIR_NAME,
};
pub use settings::{
    load_compile_check, load_compiler_path, load_config, load_config_from_path,
    load_lookup_script_roots, load_lookup_script_roots_from_path, load_script_roots,
    load_strict_achlist_scope, load_strict_achlist_scope_from_path, resolve_compiler_path,
    save_compile_check, save_compiler_path, save_config, save_config_at_path,
    save_lookup_script_roots, save_script_roots,
};

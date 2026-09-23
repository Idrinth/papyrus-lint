//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job,
//! and the app-level settings (currently just the PapyrusCompiler.exe path)
//! that live in the same file alongside it. Presets — named baseline
//! configurations `init` (or the desktop app's first-run picker) can
//! generate a project's `papyrus-lint.yaml` from — are [`presets`].
//!
//! Split by reason to change: [`project_file`] owns the YAML document
//! itself — the project-file model, discovery, and the per-field
//! loaders/savers built on it; the generated [`comments`] module injects
//! the default config's explanatory comments above each saved key;
//! [`skyrim`] and [`fallout4`] each own detecting their game's install (via
//! the Windows registry) and its vanilla script directories, with
//! [`game_install`] dispatching to whichever of them matches a project's
//! configured game and merging the result into `lookup_script_roots`;
//! [`yaml_merge`] owns recursive YAML layering; and [`compiler`] owns
//! locating `PapyrusCompiler.exe`. This module re-exports their combined
//! public API at the crate root, so callers outside this crate are unaffected
//! by the split.

mod comments {
    include!(concat!(env!("OUT_DIR"), "/comments.rs"));
}
mod compiler;
mod fallout4;
mod game_install;
pub mod presets;
mod project_file;
mod skyrim;
mod yaml_merge;

pub use compiler::{auto_detect_compiler_path, resolve_compiler_path};
pub use fallout4::{detected_fallout4_install_path, detected_fallout4_script_lookup_dirs};
pub use project_file::{
    config_file_path, lint_config_to_yaml, load_compile_check, load_compiler_path, load_config,
    load_config_from_path, load_lookup_script_roots, load_lookup_script_roots_from_path,
    load_script_roots, load_strict_achlist_scope, load_strict_achlist_scope_from_path,
    parse_lint_yaml, save_compile_check, save_compiler_path, save_config, save_config_at_path,
    save_lookup_script_roots, save_script_roots,
};
pub use skyrim::{detected_skyrim_install_path, detected_skyrim_script_lookup_dirs};

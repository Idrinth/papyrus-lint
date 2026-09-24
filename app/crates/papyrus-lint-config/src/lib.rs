//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job,
//! and the app-level compiler, script-root, and project-scope settings that
//! live in the same file alongside it. Presets — named baseline
//! configurations `init` (or the desktop app's first-run picker) can
//! generate a project's `papyrus-lint.yaml` from — are [`presets`].
//!
//! Split by reason to change: [`project_file`] owns the shared YAML document
//! model, discovery, and persistence; [`lint_config`], [`compiler_config`],
//! [`script_roots`], and [`achlist_config`] own their settings APIs and tests;
//! the generated [`comments`] module injects the default config's explanatory
//! comments above each saved key;
//! [`skyrim`] and [`fallout4`] each own detecting their game's install (via
//! the Windows registry) and its vanilla script directories, with
//! [`game_install`] dispatching to whichever of them matches a project's
//! configured game and merging the result into `lookup_script_roots`;
//! [`yaml_merge`] owns recursive YAML layering; and [`compiler`] owns
//! locating `PapyrusCompiler.exe`. This module re-exports their combined
//! public API at the crate root, so callers outside this crate are unaffected
//! by the split.

mod achlist_config;
mod comments {
    include!(concat!(env!("OUT_DIR"), "/comments.rs"));
}
mod compiler;
mod compiler_config;
mod fallout4;
mod game_install;
mod lint_config;
mod preset_files;
pub mod presets;
mod project_file;
mod script_roots;
mod skyrim;
mod yaml_merge;

pub use achlist_config::{load_strict_achlist_scope, load_strict_achlist_scope_from_path};
pub use compiler::{auto_detect_compiler_path, resolve_compiler_path};
pub use compiler_config::{
    load_compile_check, load_compiler_path, save_compile_check, save_compiler_path,
};
pub use fallout4::{detected_fallout4_install_path, detected_fallout4_script_lookup_dirs};
pub use lint_config::{
    lint_config_to_yaml, load_config, load_config_from_path, parse_lint_yaml, save_config,
    save_config_at_path,
};
pub use project_file::config_file_path;
pub use script_roots::{
    load_lookup_script_roots, load_lookup_script_roots_from_path, load_script_roots,
    save_lookup_script_roots, save_script_roots,
};
pub use skyrim::{detected_skyrim_install_path, detected_skyrim_script_lookup_dirs};

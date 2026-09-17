//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job,
//! and the app-level settings (currently just the PapyrusCompiler.exe path)
//! that live in the same file alongside it. Presets — named baseline
//! configurations `init` (or the desktop app's first-run picker) can
//! generate a project's `papyrus-lint.yaml` from — are [`presets`].
//!
//! Split by reason to change: [`project_file`] owns the YAML document
//! itself — the project-file model, discovery, and the per-field
//! loaders/savers built on it; [`comments`] owns the README-synced
//! explanatory comments injected above each saved key; [`skyrim`] owns
//! detecting a Skyrim Special Edition install (via the Windows registry)
//! and its vanilla script directories; and [`compiler`] owns locating
//! `PapyrusCompiler.exe`. This module re-exports their combined public API
//! at the crate root, so callers outside this crate are unaffected by the
//! split.

mod comments;
mod compiler;
pub mod presets;
mod project_file;
mod skyrim;

pub use compiler::{auto_detect_compiler_path, resolve_compiler_path};
pub use project_file::{
    config_file_path, lint_config_to_yaml, load_compile_check, load_compiler_path, load_config,
    load_config_from_path, load_lookup_script_roots, load_lookup_script_roots_from_path,
    load_script_roots, load_strict_achlist_scope, load_strict_achlist_scope_from_path,
    parse_lint_yaml, save_compile_check, save_compiler_path, save_config, save_config_at_path,
    save_lookup_script_roots, save_script_roots,
};
pub use skyrim::{detected_skyrim_install_path, detected_skyrim_script_lookup_dirs};

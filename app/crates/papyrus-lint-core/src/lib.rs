//! Project-level logic shared by the Papyrus Lint desktop app and its CLI:
//! parsing `.achlist` files, resolving `.psc` sources by name, and building
//! a cross-script function signature table for the "Argument type check"/
//! "Return type check" lints. Locating/loading a project's `papyrus-lint`
//! YAML config lives in the separate [`papyrus_lint_config`] crate this one
//! depends on (see [`presets`], which layers the desktop app's first-run
//! preset picker metadata over [`papyrus_lint_config::Preset`]).
//!
//! This crate depends only on [`papyrus_parser`], [`papyrus_lints`], and
//! [`papyrus_lint_config`], not on Tauri, so it can be reused by anything
//! that needs to lint a project's scripts without pulling in the desktop
//! app.

pub mod achlist;
pub mod ast_cache;
pub mod compile_diagnostics;
pub mod compiler;
pub mod content_hash;
pub mod diff;
pub mod function_table;
mod native_globals;
mod native_types;
pub mod parallel;
pub mod pex_header;
pub mod presets;
pub mod script_filename_mismatch;
mod script_functions;
pub mod script_locator;
pub mod source_encoding;
pub mod stale_pex;

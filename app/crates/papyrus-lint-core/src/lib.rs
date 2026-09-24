//! Project-level logic shared by the Papyrus Lint desktop app and its CLI:
//! parsing `.achlist` files, resolving `.psc` sources by name, and building
//! a cross-script function signature table for the "Argument type check"/
//! "Return type check" lints. Locating/loading a project's `papyrus-lint`
//! YAML config lives in the separate [`papyrus_lint_config`] crate this one
//! depends on (see [`presets`], which layers the desktop app's first-run
//! preset picker metadata over [`papyrus_lint_config::Preset`]).
//!
//! This crate depends only on [`papyrus_parser`], [`papyrus_lints`],
//! [`papyrus_lint_config`], and [`papyrus_ast_cache`], not on Tauri, so it
//! can be reused by anything that needs to lint a project's scripts
//! without pulling in the desktop app.

pub mod achlist;
/// Disk-backed AST/token cache. Re-exported from the standalone
/// [`papyrus_ast_cache`] crate, which owns the implementation -- it's
/// self-contained enough (only depending on [`papyrus_parser`]) to be
/// reusable outside this crate too.
pub use papyrus_ast_cache as ast_cache;
pub mod compile_diagnostics;
pub mod compiler;
pub mod content_hash;
pub mod diff;
pub mod function_table;
mod native_globals;
pub mod parallel;
pub mod pex_header;
pub mod ppj;
pub mod presets;
pub mod project_root;
mod script_functions;
pub mod script_locator;
pub mod source_encoding;
pub mod stale_pex;

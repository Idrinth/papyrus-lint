//! Project-level logic shared by the Papyrus Lint desktop app and its CLI:
//! parsing `.achlist` files, locating/loading a project's `papyrus-lint`
//! YAML config, resolving `.psc` sources by name, and building a
//! cross-script function signature table for the "Argument type check"/
//! "Return type check" lints.
//!
//! This crate depends only on [`papyrus_parser`] and [`papyrus_lints`], not
//! on Tauri, so it can be reused by anything that needs to lint a project's
//! scripts without pulling in the desktop app.

pub mod achlist;
pub mod config;
pub mod function_table;
mod native_types;
pub mod script_locator;
pub mod source_encoding;

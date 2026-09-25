//! Shared constants and types used across Papyrus Lint crates.
//!
//! This crate is the leaf every other reusable crate depends on for values
//! that must stay identical at every call site. Adding a new global here is
//! cheaper than threading a string or a per-crate enum through parser,
//! cache, lints, config, core, CLI, and the desktop shell.

mod game;

pub use game::Game;

/// The project's canonical website root, shared by documentation links and
/// machine-readable reports.
pub const WEBSITE_URL: &str = "https://papyrus-lint.idrinth.de";

//! Stdio Language Server Protocol adapter for Papyrus Lint.
//!
//! Document sync publishes diagnostics from `papyrus_lints`. Code actions
//! offer the per-diagnostic fix and ignore edits. `workspace/executeCommand`
//! is still empty.

mod code_actions;
mod diagnostics;
mod documents;
mod framing;
mod server;

pub use server::{serve, FIX_FILE_COMMAND};

pub const SERVER_NAME: &str = "papyrus-lint-lsp";

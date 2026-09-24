//! Stdio Language Server Protocol adapter for Papyrus Lint.
//!
//! Document sync publishes diagnostics from `papyrus_lints`. Code actions and
//! `workspace/executeCommand` are still empty until those follow-ups land.

mod diagnostics;
mod documents;
mod framing;
mod server;

pub use server::{serve, FIX_FILE_COMMAND};

pub const SERVER_NAME: &str = "papyrus-lint-lsp";

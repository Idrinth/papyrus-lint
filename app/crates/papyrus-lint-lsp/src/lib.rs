//! Stdio Language Server Protocol adapter for Papyrus Lint.
//!
//! Document sync publishes diagnostics from `papyrus_lints`. Code actions
//! edit one diagnostic. `workspace/executeCommand` applies every automatic
//! fix in the open file.

mod code_actions;
mod commands;
mod diagnostics;
mod documents;
mod framing;
mod server;

pub use server::{serve, FIX_FILE_COMMAND};

pub const SERVER_NAME: &str = "papyrus-lint-lsp";

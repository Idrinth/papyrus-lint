//! Stdio Language Server Protocol adapter for Papyrus Lint.
//!
//! v1 speaks the handshake and accepts the document-sync, code-action, and
//! execute-command messages editors will send. It does not lint, publish
//! diagnostics, or apply fixes yet.

mod framing;
mod server;

pub use server::{serve, FIX_FILE_COMMAND};

pub const SERVER_NAME: &str = "papyrus-lint-lsp";

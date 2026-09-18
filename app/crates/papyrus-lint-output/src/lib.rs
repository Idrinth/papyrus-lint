//! Plain-text/JSON/AI-export report formatting, shared by the standalone
//! `PapyrusLinterCLI` binary (`papyrus-lint-cli`, which formats
//! `papyrus_lints::Diagnostic`s straight from its own lint pass) and the
//! desktop app's Tauri commands (`app/src-tauri`, which reformat a
//! diagnostic list the frontend sends back over Tauri's IPC boundary as
//! plain JSON - see [`DiagnosticLike`]/[`OwnedDiagnostic`]) - so a
//! diagnostic's exported shape, and the AI export's schema in particular,
//! can never drift between the CLI and the GUI.
//!
//! This crate only formats. Scanning a project, running the lint engine
//! itself, and deciding what counts as a run failure all stay with each
//! caller.

mod ai;
mod diagnostic;
mod json;
mod plain;

pub use ai::*;
pub use diagnostic::*;
pub use json::*;
pub use plain::*;

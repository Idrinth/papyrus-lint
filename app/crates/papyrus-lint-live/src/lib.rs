//! In-memory ("live" / blob) Papyrus linting.
//!
//! [`lint_source`] runs the lint engine against a buffer of source text
//! instead of a project on disk: no achlist/`.ppj` resolution, no
//! cross-script `FunctionTable`, and no project-level lints such as
//! `conflicting_script_versions` / `stale_compiled_output` /
//! `script_filename_mismatch`. That is the same contract
//! `PapyrusLinterCLI --blob`, the language server's open-document snapshot,
//! and the desktop code viewer's live edit (`lint_papyrus_script`) all
//! need, so those callers use this crate instead of each reimplementing
//! the pass.
//!
//! Configuration is resolved separately: [`config_from_override`] is the
//! `--config <path>` / "use this file or the engine default" path, and
//! [`config_from_script_path`] walks parent directories of an on-disk
//! script looking for `papyrus-lint.yaml`/`.yml`.

mod analysis;
mod config;

pub use analysis::{lint_source, LiveAnalysis, ParserFailure, ParserFailureKind};
pub use config::{config_from_override, config_from_script_path};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

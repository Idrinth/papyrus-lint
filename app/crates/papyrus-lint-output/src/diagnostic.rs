use serde::{Deserialize, Serialize};

/// Anything that can be formatted as one lint diagnostic. Implemented by
/// `papyrus_lints::Diagnostic` itself, so `papyrus-lint-cli` (which always
/// lints in the same process it reports from) can format its own
/// diagnostics with no conversion step, and by [`OwnedDiagnostic`], for a
/// caller (the desktop app's Tauri commands) whose diagnostics instead
/// arrive as plain JSON over an IPC boundary and so can't carry
/// [`papyrus_lints::Diagnostic::rule`]'s `&'static str`.
pub trait DiagnosticLike {
    fn line(&self) -> usize;
    fn column(&self) -> usize;
    fn rule(&self) -> &str;
    fn message(&self) -> &str;
}

impl DiagnosticLike for papyrus_lints::Diagnostic {
    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn rule(&self) -> &str {
        self.rule
    }

    fn message(&self) -> &str {
        &self.message
    }
}

/// One diagnostic as sent back over Tauri's IPC boundary: the desktop
/// app's own already-linted findings, handed back to a `format_issues_*`
/// command (see `app/src-tauri/src/export.rs`) to render as plain text,
/// JSON, or an AI export, the same way `papyrus-lint-cli` renders its own
/// `papyrus_lints::Diagnostic`s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnedDiagnostic {
    pub line: usize,
    pub column: usize,
    pub rule: String,
    pub message: String,
}

impl DiagnosticLike for OwnedDiagnostic {
    fn line(&self) -> usize {
        self.line
    }

    fn column(&self) -> usize {
        self.column
    }

    fn rule(&self) -> &str {
        &self.rule
    }

    fn message(&self) -> &str {
        &self.message
    }
}

/// The severity level tagged onto the front of `message` (e.g. `"[warning]
/// ..."`). Mirrors `papyrus_lints::Diagnostic::level`, generalized to a
/// bare message string so a caller that only has an [`OwnedDiagnostic`] in
/// hand doesn't need the original `papyrus_lints::Diagnostic` too. Every
/// built-in lint tags one; an untagged message is classified as an error so
/// every diagnostic has a level and an accidentally omitted tag cannot make
/// a potentially serious finding less visible.
pub fn level_of(message: &str) -> &'static str {
    if message.starts_with("[error]") {
        "error"
    } else if message.starts_with("[warning]") {
        "warning"
    } else if message.starts_with("[info]") {
        "info"
    } else {
        "error"
    }
}

/// Strips a leading `[error]`/`[warning]`/`[info]` severity tag (and any
/// whitespace right after it) from `message`, for the AI export's
/// `strip_severity_prefix` option (see [`crate::to_json_diagnostics`]),
/// which carries the level separately as its own `level` field and so
/// avoids repeating it inside `message` too. A message with no such prefix
/// is returned unchanged.
pub fn strip_severity_prefix(message: &str) -> String {
    for tag in ["[error]", "[warning]", "[info]"] {
        if let Some(rest) = message.strip_prefix(tag) {
            return rest.trim_start().to_string();
        }
    }
    message.to_string()
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;

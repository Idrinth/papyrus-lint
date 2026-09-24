//! Detects byte-different, same-named scripts in a project file snapshot.

use std::path::{Path, PathBuf};

use crate::Diagnostic;

pub const RULE: &str = "conflicting-script-versions";

/// One file in the complete set visible to a project lint run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFile {
    pub path: PathBuf,
    pub display_path: String,
    pub content_md5: String,
}

/// Checks `current_path` against the supplied project file snapshot.
///
/// Files whose names differ are ignored. Supplying the whole snapshot keeps
/// project discovery in the caller while leaving the diagnostic policy in the
/// lint rule that owns it. Content is compared by MD5 so callers can reuse a
/// digest already stored in the AST cache instead of reopening each `.psc`.
pub fn check(
    current_path: &Path,
    current_content_md5: &str,
    files: &[ProjectFile],
) -> Vec<Diagnostic> {
    let Some(file_name) = current_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };

    let mut conflicts: Vec<&ProjectFile> = files
        .iter()
        .filter(|candidate| candidate.path != current_path)
        .filter(|candidate| {
            candidate
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
        })
        .filter(|candidate| candidate.content_md5 != current_content_md5)
        .collect();
    conflicts.sort_by(|left, right| left.path.cmp(&right.path));
    conflicts.dedup_by(|left, right| left.path == right.path);

    conflicts
        .into_iter()
        .map(|candidate| Diagnostic {
            line: 1,
            column: 1,
            rule: RULE,
            message: format!(
                "[warning] A different version of {file_name} is also available at {}; script resolution may depend on search-directory order",
                candidate.display_path
            ),
        })
        .collect()
}

#[cfg(test)]
#[path = "conflicting_script_versions_tests.rs"]
mod tests;

//! Project-level, per-line diagnostic suppressions from `.papyrus-lint-ignore`.

use std::fs;
use std::path::{Path, PathBuf};

use papyrus_lints::Diagnostic;
use serde::Deserialize;

pub const IGNORE_FILE_NAME: &str = ".papyrus-lint-ignore";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IgnoreEntry {
    file: PathBuf,
    line: usize,
    rule: String,
}

/// Parsed project ignores. Relative `file` values are resolved from the
/// project root; absolute values are used as written.
#[derive(Debug)]
pub struct IgnoreFile {
    entries: Vec<IgnoreEntry>,
    project_root: PathBuf,
}

impl IgnoreFile {
    /// Loads `.papyrus-lint-ignore` from `project_root` when it exists.
    /// A missing file returns `None`; malformed YAML and zero line numbers
    /// are reported only when a file is present.
    pub fn load_optional(project_root: &Path) -> Result<Option<Self>, String> {
        let path = project_root.join(IGNORE_FILE_NAME);
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(format!("failed to read {}: {err}", path.display())),
        };
        let entries: Vec<IgnoreEntry> = serde_norway::from_str(&source)
            .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
        if entries.iter().any(|entry| entry.line == 0) {
            return Err(format!(
                "failed to parse {}: line numbers must be 1 or greater",
                path.display()
            ));
        }
        Ok(Some(Self {
            entries,
            project_root: project_root.to_path_buf(),
        }))
    }

    /// Removes diagnostics suppressed for `file`, matching rule ids and
    /// 1-indexed diagnostic lines exactly.
    pub fn retain_diagnostics(&self, file: &Path, diagnostics: &mut Vec<Diagnostic>) {
        diagnostics.retain(|diagnostic| {
            !self.entries.iter().any(|entry| {
                entry.line == diagnostic.line
                    && entry.rule == diagnostic.rule
                    && paths_match(&self.project_root, &entry.file, file)
            })
        });
    }
}

fn paths_match(project_root: &Path, configured: &Path, actual: &Path) -> bool {
    let configured = if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        project_root.join(configured)
    };
    match (configured.canonicalize(), actual.canonicalize()) {
        (Ok(configured), Ok(actual)) => configured == actual,
        _ => configured == actual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(line: usize, rule: &'static str) -> Diagnostic {
        Diagnostic {
            line,
            column: 1,
            message: "test".to_string(),
            rule,
        }
    }

    #[test]
    fn loads_relative_and_absolute_ignores_and_matches_only_exact_findings() {
        let root = tempfile::tempdir().unwrap();
        let relative = root.path().join("scripts/Relative.psc");
        let absolute = root.path().join("Absolute.psc");
        fs::create_dir_all(relative.parent().unwrap()).unwrap();
        fs::write(&relative, "").unwrap();
        fs::write(&absolute, "").unwrap();
        fs::write(
            root.path().join(IGNORE_FILE_NAME),
            format!(
                "- file: scripts/Relative.psc\n  line: 12\n  rule: trailing-whitespace\n- file: {}\n  line: 4\n  rule: semicolon\n",
                absolute.display()
            ),
        )
        .unwrap();

        let ignores = IgnoreFile::load_optional(root.path()).unwrap().unwrap();
        let mut relative_diagnostics = vec![
            diagnostic(12, "trailing-whitespace"),
            diagnostic(13, "trailing-whitespace"),
            diagnostic(12, "semicolon"),
        ];
        ignores.retain_diagnostics(&relative, &mut relative_diagnostics);
        assert_eq!(relative_diagnostics.len(), 2);

        let mut absolute_diagnostics = vec![diagnostic(4, "semicolon")];
        ignores.retain_diagnostics(&absolute, &mut absolute_diagnostics);
        assert!(absolute_diagnostics.is_empty());
    }

    #[test]
    fn missing_file_is_empty_and_invalid_input_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert!(IgnoreFile::load_optional(root.path()).unwrap().is_none());
        fs::write(
            root.path().join(IGNORE_FILE_NAME),
            "- file: Example.psc\n  line: 0\n  rule: trailing-whitespace\n",
        )
        .unwrap();
        assert!(IgnoreFile::load_optional(root.path()).is_err());
    }
}

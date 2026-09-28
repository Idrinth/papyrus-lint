//! Infer the script search root that should own a discovered `.psc` path.
//!
//! Scan and achlist discovery add directories so listed scripts can resolve
//! each other. Nested files under a conventional `scripts/source` or
//! `source/scripts` tree belong to that tree. Using the immediate parent
//! would index `User/Foo.psc` as the unqualified `foo.psc`. Namespace lookup
//! is resolution's job; the script index must not pre-list those leaf-name
//! aliases.

use std::path::{Path, PathBuf};

/// Search root that should own `script_path` when scan or achlist discovery
/// adds directories so those scripts can resolve each other.
///
/// Scripts outside a conventional tree still use their immediate parent, so
/// a flat custom folder listed by an achlist keeps working.
pub fn inferred_script_search_root(script_path: &Path, project_root: &Path) -> PathBuf {
    let Some(parent) = script_path.parent() else {
        return project_root.to_path_buf();
    };
    let mut current = parent;
    loop {
        if is_conventional_script_root(current) {
            return current.to_path_buf();
        }
        if current == project_root {
            break;
        }
        match current.parent() {
            Some(next) => current = next,
            None => break,
        }
    }
    parent.to_path_buf()
}

fn is_conventional_script_root(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(parent_name) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    (name.eq_ignore_ascii_case("source") && parent_name.eq_ignore_ascii_case("scripts"))
        || (name.eq_ignore_ascii_case("scripts") && parent_name.eq_ignore_ascii_case("source"))
}

#[cfg(test)]
#[path = "script_search_root_tests.rs"]
mod tests;

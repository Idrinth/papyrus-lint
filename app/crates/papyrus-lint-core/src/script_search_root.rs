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

/// Path of `script_path` relative to a conventional `scripts/source` or
/// `source/scripts` ancestor.
///
/// Used when configured search roots fail to strip (Windows case mismatch
/// against `scripts/source`) so a namespaced ScriptName is not compared to
/// the leaf filename alone.
pub fn relative_path_from_inferred_root(
    script_path: &Path,
    project_root: &Path,
) -> Option<PathBuf> {
    let inferred = inferred_script_search_root(script_path, project_root);
    if !is_conventional_script_root(&inferred) {
        return None;
    }
    strip_prefix_ignore_ascii_case(script_path, &inferred)
}

/// `path` with `prefix` removed, comparing each normal component
/// case-insensitively. `None` when `path` is not under `prefix`.
pub(crate) fn strip_prefix_ignore_ascii_case(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let path_components: Vec<_> = path.components().collect();
    let prefix_components: Vec<_> = prefix.components().collect();
    if path_components.len() < prefix_components.len() {
        return None;
    }
    for (actual, expected) in path_components.iter().zip(&prefix_components) {
        match (actual, expected) {
            (std::path::Component::Normal(actual), std::path::Component::Normal(expected)) => {
                if !actual.to_str()?.eq_ignore_ascii_case(expected.to_str()?) {
                    return None;
                }
            }
            _ if actual == expected => {}
            _ => return None,
        }
    }
    let rest = path_components[prefix_components.len()..]
        .iter()
        .collect::<PathBuf>();
    if rest.as_os_str().is_empty() {
        None
    } else {
        Some(rest)
    }
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

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
mod tests {
    use super::inferred_script_search_root;
    use std::fs;
    use std::path::PathBuf;

    fn write_file(dir: &std::path::Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "").expect("failed to write test script file");
        path
    }

    #[test]
    fn uses_conventional_tree_not_nested_parent() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source = root.path().join("Scripts/Source/DLC03/Fragments/Quests");
        fs::create_dir_all(&source).expect("failed to create namespaced dir");
        let script = write_file(&source, "QF_Example.psc");

        assert_eq!(
            inferred_script_search_root(&script, root.path()),
            root.path().join("Scripts/Source")
        );
    }

    #[test]
    fn keeps_flat_custom_parent() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let custom = root.path().join("mods/MyMod");
        fs::create_dir_all(&custom).expect("failed to create custom dir");
        let script = write_file(&custom, "MyQuest.psc");

        assert_eq!(inferred_script_search_root(&script, root.path()), custom);
    }

    #[test]
    fn accepts_source_scripts_casing() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let nested = root.path().join("Source/Scripts/User");
        fs::create_dir_all(&nested).expect("failed to create namespace dir");
        let script = write_file(&nested, "Foo.psc");

        assert_eq!(
            inferred_script_search_root(&script, root.path()),
            root.path().join("Source/Scripts")
        );
    }
}

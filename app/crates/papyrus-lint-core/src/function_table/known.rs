//! Name keys used by [`super::FunctionTable::with_known_scripts`].

use std::path::{Path, PathBuf};

use crate::script_locator::CANDIDATE_DIRS;

/// Keys under which `path` is registered in known-scripts mode: the file
/// stem, a path-derived `folder:stem` name when the file sits under a
/// script root, and the declared `ScriptName` when that header can be
/// read without a full parse.
pub(super) fn known_script_keys(
    root: &Path,
    additional_roots: &[String],
    path: &Path,
) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        keys.push(stem.to_ascii_lowercase());
    }
    if let Some(qualified) = path_derived_script_name(root, additional_roots, path) {
        if !keys.iter().any(|key| key == &qualified) {
            keys.push(qualified);
        }
    }
    if let Some(declared) = declared_script_name(path) {
        if !keys.iter().any(|key| key == &declared) {
            keys.push(declared);
        }
    }
    keys
}

/// `Scripts/Source/User/Quests/Foo.psc` → `user:quests:foo` relative to a
/// conventional or configured script root. A file that lives directly in a
/// root has no extra segments and is left to the stem key.
fn path_derived_script_name(
    root: &Path,
    additional_roots: &[String],
    path: &Path,
) -> Option<String> {
    let mut roots: Vec<PathBuf> = CANDIDATE_DIRS.iter().map(|dir| root.join(dir)).collect();
    roots.extend(crate::script_locator::resolve_additional_roots(
        root,
        additional_roots,
    ));
    for search_root in roots {
        if let Ok(relative) = path.strip_prefix(&search_root) {
            return relative_psc_to_qualified_name(relative);
        }
    }
    None
}

fn relative_psc_to_qualified_name(relative: &Path) -> Option<String> {
    let mut parts: Vec<String> = relative
        .iter()
        .filter_map(|component| component.to_str())
        .map(str::to_ascii_lowercase)
        .collect();
    let last = parts.last_mut()?;
    if let Some(stem) = last.strip_suffix(".psc") {
        *last = stem.to_string();
    }
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(parts.join(":"))
}

fn declared_script_name(path: &Path) -> Option<String> {
    let source = crate::source_encoding::read_psc_source(path).ok()?;
    peek_declared_script_name(&source)
}

/// First `ScriptName` token on a header line, including colon segments.
fn peek_declared_script_name(source: &str) -> Option<String> {
    for line in source.lines().take(40) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        let rest = trimmed
            .strip_prefix("ScriptName ")
            .or_else(|| trimmed.strip_prefix("Scriptname "))
            .or_else(|| trimmed.strip_prefix("scriptname "))
            .or_else(|| trimmed.strip_prefix("SCRIPTNAME "))?;
        let name = rest.split_whitespace().next()?;
        if name.is_empty() {
            return None;
        }
        return Some(name.to_ascii_lowercase());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::FunctionTable;
    use super::peek_declared_script_name;
    use std::fs;

    #[test]
    fn peek_reads_a_namespaced_script_name() {
        assert_eq!(
            peek_declared_script_name("; header\nScriptName User:Foo Extends Quest\n"),
            Some("user:foo".to_string())
        );
    }

    #[test]
    fn with_known_scripts_registers_qualified_names_without_collapsing_stems() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let user_dir = root.path().join("scripts/source/User");
        let other_dir = root.path().join("scripts/source/Other");
        fs::create_dir_all(&user_dir).expect("failed to create User namespace dir");
        fs::create_dir_all(&other_dir).expect("failed to create Other namespace dir");
        let user_foo = user_dir.join("Foo.psc");
        let other_foo = other_dir.join("Foo.psc");
        fs::write(
            &user_foo,
            "ScriptName User:Foo\n\nFunction FromUser()\nEndFunction\n",
        )
        .expect("failed to write User:Foo");
        fs::write(
            &other_foo,
            "ScriptName Other:Foo\n\nFunction FromOther()\nEndFunction\n",
        )
        .expect("failed to write Other:Foo");

        let mut table = FunctionTable::new(root.path().to_path_buf())
            .with_game(papyrus_lints::Game::Fallout4)
            .with_known_scripts(&[user_foo, other_foo]);

        assert!(table.script_exists("User:Foo"));
        assert!(table.script_exists("Other:Foo"));
        assert!(table.script_exists("Foo"));
        assert!(table.lookup_function("User:Foo", "FromUser").is_some());
        assert!(table.lookup_function("Other:Foo", "FromOther").is_some());
        assert!(table.lookup_function("User:Foo", "FromOther").is_none());
        assert!(table.lookup_function("Foo", "FromUser").is_some());
    }

    #[test]
    fn with_known_scripts_loads_a_namespaced_parent_for_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let user_dir = root.path().join("scripts/source/User");
        fs::create_dir_all(&user_dir).expect("failed to create User namespace dir");
        let parent = user_dir.join("Base.psc");
        let child = user_dir.join("Child.psc");
        fs::write(
            &parent,
            "ScriptName User:Base\n\nFunction FromParent()\nEndFunction\n",
        )
        .expect("failed to write namespaced parent");
        fs::write(&child, "ScriptName User:Child Extends User:Base\n")
            .expect("failed to write namespaced child");

        let mut table = FunctionTable::new(root.path().to_path_buf())
            .with_game(papyrus_lints::Game::Fallout4)
            .with_known_scripts(&[parent, child]);

        let signature = table
            .lookup_function("User:Child", "FromParent")
            .expect("namespaced Extends should load the qualified parent");
        assert_eq!(signature.name, "FromParent");
    }
}

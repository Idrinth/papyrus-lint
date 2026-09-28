//! Unqualified script names that refer to a peer of the script being linted.
//!
//! Nested files stay out of the global leaf-name index (#1522), so
//! `User/Foo.psc` does not collide with `Other/Foo.psc`. While a script is
//! being linted, a bare name may still mean a file in that script's own
//! directory, or the only file of that stem under its namespace folder
//! (`CreationClub/**/VRWorkshopParentScript.psc` from a fragment in that
//! namespace). That lookup is not an index key and is not a conflict.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::load::ScriptOrigin;

thread_local! {
    static PEER_SCOPE: RefCell<Option<PeerScope>> = const { RefCell::new(None) };
    /// Stems under a namespace folder. `None` means more than one file shared
    /// that stem, so a bare name must not pick one.
    static PACKAGE_LEAVES: RefCell<HashMap<PathBuf, HashMap<String, Option<PathBuf>>>> =
        RefCell::new(HashMap::new());
}

#[derive(Clone)]
struct PeerScope {
    directory: PathBuf,
    /// First `ScriptName` segment when the name is qualified. Empty otherwise.
    namespace_root: String,
}

#[derive(Clone)]
pub(super) struct LeafHit {
    pub path: PathBuf,
    pub origin: ScriptOrigin,
}

/// Remembers `script_path` as the script being linted until the guard drops.
/// Nested calls restore the previous script.
pub(crate) fn enter_peer_scope(script_path: &Path, source: &str) -> PeerScopeGuard {
    let scope = PeerScope {
        directory: script_path.parent().unwrap_or(script_path).to_path_buf(),
        namespace_root: namespace_root(source),
    };
    let previous = PEER_SCOPE.with(|slot| slot.borrow_mut().replace(scope));
    PeerScopeGuard { previous }
}

pub(crate) struct PeerScopeGuard {
    previous: Option<PeerScope>,
}

impl Drop for PeerScopeGuard {
    fn drop(&mut self) {
        PEER_SCOPE.with(|slot| {
            *slot.borrow_mut() = self.previous.take();
        });
    }
}

pub(super) fn push_index_leaves(
    index: &crate::script_locator::ScriptIndex,
    origin: ScriptOrigin,
    leaves: &mut HashMap<String, Vec<LeafHit>>,
) {
    for paths in index.values() {
        for path in paths {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let stem = stem.to_ascii_lowercase();
            let bucket = leaves.entry(stem).or_default();
            if bucket.iter().any(|hit| hit.path == *path) {
                continue;
            }
            bucket.push(LeafHit {
                path: path.clone(),
                origin,
            });
        }
    }
}

/// Resolves an unqualified `name_lower` against the script being linted.
///
/// `indexed` is every stem already recorded from a script index (project
/// and lookup). When it is `None`, project files are not indexed and the
/// referrer's directory is read from disk instead. Lookup-index stems, if
/// any, are still passed in `indexed`.
pub(super) fn resolve_peer_path(
    name_lower: &str,
    indexed: Option<&HashMap<String, Vec<LeafHit>>>,
    search_filesystem: bool,
) -> Option<LeafHit> {
    if name_lower.contains(':') || name_lower.contains('/') || name_lower.contains('\\') {
        return None;
    }
    let scope = PEER_SCOPE.with(|slot| slot.borrow().clone())?;
    let stem = name_lower
        .strip_suffix(".psc")
        .unwrap_or(name_lower)
        .to_ascii_lowercase();
    if stem.is_empty() {
        return None;
    }

    let mut candidates = indexed
        .and_then(|leaves| leaves.get(&stem))
        .cloned()
        .unwrap_or_default();
    if search_filesystem {
        candidates.extend(filesystem_candidates(&scope, &stem));
    }
    pick_peer(&scope, candidates)
}

fn pick_peer(scope: &PeerScope, mut candidates: Vec<LeafHit>) -> Option<LeafHit> {
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    candidates.dedup_by(|left, right| left.path == right.path);

    if let Some(hit) = candidates
        .iter()
        .find(|hit| parent_eq(&hit.path, &scope.directory))
    {
        return Some(hit.clone());
    }
    let package = deepest_namespace_dir(&scope.directory, &scope.namespace_root)?;
    let in_package: Vec<_> = candidates
        .into_iter()
        .filter(|hit| hit.path.starts_with(&package))
        .collect();
    if in_package.len() == 1 {
        return in_package.into_iter().next();
    }
    None
}

fn filesystem_candidates(scope: &PeerScope, stem: &str) -> Vec<LeafHit> {
    let mut hits = Vec::new();
    if let Some(path) = leaf_in_dir(&scope.directory, stem) {
        hits.push(LeafHit {
            path,
            origin: ScriptOrigin::Project,
        });
        return hits;
    }
    let Some(package) = deepest_namespace_dir(&scope.directory, &scope.namespace_root) else {
        return hits;
    };
    if parent_eq_paths(&package, &scope.directory) {
        return hits;
    }
    if let Some(path) = unique_leaf_under(&package, stem) {
        hits.push(LeafHit {
            path,
            origin: ScriptOrigin::Project,
        });
    }
    hits
}

fn leaf_in_dir(dir: &Path, stem: &str) -> Option<PathBuf> {
    let expected = format!("{stem}.psc");
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            (name.eq_ignore_ascii_case(&expected) && path.is_file()).then_some(path)
        })
}

fn unique_leaf_under(package: &Path, stem: &str) -> Option<PathBuf> {
    PACKAGE_LEAVES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let leaves = cache
            .entry(package.to_path_buf())
            .or_insert_with(|| index_package(package));
        leaves.get(stem).cloned().flatten()
    })
}

fn index_package(package: &Path) -> HashMap<String, Option<PathBuf>> {
    let mut leaves: HashMap<String, Option<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(package)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.into_path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.to_ascii_lowercase().ends_with(".psc") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let stem = stem.to_ascii_lowercase();
        match leaves.entry(stem) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Some(path));
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if entry
                    .get()
                    .as_ref()
                    .is_some_and(|existing| existing != &path)
                {
                    entry.insert(None);
                }
            }
        }
    }
    leaves
}

fn namespace_root(source: &str) -> String {
    for line in source.lines().take(40) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(keyword) = parts.next() else {
            return String::new();
        };
        if !keyword.eq_ignore_ascii_case("scriptname") {
            return String::new();
        }
        let Some(name) = parts.next() else {
            return String::new();
        };
        let Some((root, _)) = name.split_once(':') else {
            return String::new();
        };
        return root.to_ascii_lowercase();
    }
    String::new()
}

/// Deepest directory at or above `script_dir` whose name is `namespace_root`.
fn deepest_namespace_dir(script_dir: &Path, namespace_root: &str) -> Option<PathBuf> {
    if namespace_root.is_empty() {
        return None;
    }
    let mut current = script_dir;
    loop {
        if current
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(namespace_root))
        {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

fn parent_eq(path: &Path, directory: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| parent_eq_paths(parent, directory))
}

fn parent_eq_paths(left: &Path, right: &Path) -> bool {
    let left: Vec<_> = left.components().collect();
    let right: Vec<_> = right.components().collect();
    left.len() == right.len()
        && left
            .iter()
            .zip(&right)
            .all(|(left, right)| match (left, right) {
                (std::path::Component::Normal(left), std::path::Component::Normal(right)) => {
                    left.eq_ignore_ascii_case(right)
                }
                _ => left == right,
            })
}

#[cfg(test)]
#[path = "peer_tests.rs"]
mod tests;

//! Locates Papyrus `.psc` source files by case-insensitive name.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use papyrus_lint_globals::Game;
use papyrus_lints::conflicting_script_versions::ProjectFile;
use walkdir::WalkDir;

use crate::collision_cache;
use crate::project_root::display_path;

/// Rule id used when multiple search roots contain different versions of
/// the same script.
pub const CONFLICTING_SCRIPT_VERSIONS_RULE: &str = "conflicting-script-versions";

/// Reads a complete path snapshot for project-level lint rules. Files that
/// disappear or become unreadable between discovery and linting are omitted.
/// Prefers a still-fresh content hash from the script-collision cache so a
/// previously seen `.psc` does not need to be opened again just to hash it.
pub fn project_files(
    paths: impl IntoIterator<Item = PathBuf>,
    root: &Path,
    short_paths: bool,
    game: Game,
) -> Vec<ProjectFile> {
    paths
        .into_iter()
        .filter_map(|path| {
            let content_hash = file_content_hash(&path, game)?;
            Some(ProjectFile {
                display_path: display_path(&path, root, short_paths),
                path,
                content_hash,
            })
        })
        .collect()
}

fn file_content_hash(path: &Path, game: Game) -> Option<String> {
    collision_cache::content_hash_for(game, path)
}

/// Directories, relative to a project root, conventionally used to store
/// Papyrus script sources. Also used by the desktop app's `compiler`
/// module to build the compiler's `-i` argument.
pub const CANDIDATE_DIRS: [&str; 2] = ["scripts/source", "source/scripts"];

/// Searches `root/scripts/source`, `root/source/scripts`, and then each of
/// `additional_roots` (in order) for a `.psc` file matching `name`,
/// case-insensitively. `name` may be given with or without the `.psc`
/// extension. Each entry in `additional_roots` is resolved relative to
/// `root` unless it's already absolute (see [`resolve_additional_roots`]).
///
/// Returns the path to the first match found, or `None` if none of those
/// locations contains a matching file. Analysis-only lookup directories
/// (see [`find_psc_file_in_lookup_roots`]) are not searched here.
pub fn find_psc_file(root: &Path, name: &str, additional_roots: &[String]) -> Option<PathBuf> {
    find_named_psc(
        CANDIDATE_DIRS
            .iter()
            .map(|dir| root.join(dir))
            .chain(resolve_additional_roots(root, additional_roots)),
        name,
    )
}

/// Searches only `lookup_roots` (resolved like [`resolve_additional_roots`])
/// for a `.psc` file matching `name`. Used as a last-resort fallback after
/// [`find_psc_file`] when resolving scripts for analysis; these directories
/// are never linted and are never fed to [`conflicting_script_versions`].
pub fn find_psc_file_in_lookup_roots(
    root: &Path,
    name: &str,
    lookup_roots: &[String],
) -> Option<PathBuf> {
    find_named_psc(resolve_additional_roots(root, lookup_roots), name)
}

fn find_named_psc(dirs: impl IntoIterator<Item = PathBuf>, name: &str) -> Option<PathBuf> {
    let name_lower = name.to_ascii_lowercase();
    let target = if name_lower.ends_with(".psc") {
        name_lower
    } else {
        format!("{name_lower}.psc")
    };

    for dir in dirs {
        for path in dir_children(&dir) {
            let matches = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|file_name| file_name.to_ascii_lowercase() == target);

            if matches && path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Immediate children of `dir`. An unreadable directory yields no entries,
/// matching the previous `fs::read_dir` + `flatten` behavior.
fn dir_children(dir: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
}

/// Resolves each of `roots` against `root`: an absolute entry is used as-is,
/// a relative one is joined onto `root`. A root ending in `source/scripts`
/// or `scripts/source` is followed by its counterpart beneath the same
/// parent, so projects containing both layouts need only configure or infer
/// one of them. Paths that are not existing directories are omitted. Used to
/// turn a project's
/// user-configured `additional_script_roots` (see
/// [`papyrus_lint_config::load_script_roots`]) into directories to search
/// alongside [`CANDIDATE_DIRS`].
pub fn resolve_additional_roots(root: &Path, roots: &[String]) -> Vec<PathBuf> {
    let mut resolved = Vec::new();

    for entry in roots {
        let path = {
            let path = Path::new(entry);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root.join(path)
            }
        };

        if path.is_dir() && !resolved.contains(&path) {
            resolved.push(path.clone());
        }

        let components = path
            .file_name()
            .and_then(|component| component.to_str())
            .zip(
                path.parent()
                    .and_then(Path::file_name)
                    .and_then(|component| component.to_str()),
            );
        let Some(pair_root) = path.parent().and_then(Path::parent) else {
            continue;
        };
        let counterpart = match components {
            Some((scripts, source))
                if scripts.eq_ignore_ascii_case("scripts")
                    && source.eq_ignore_ascii_case("source") =>
            {
                pair_root.join("scripts/source")
            }
            Some((source, scripts))
                if source.eq_ignore_ascii_case("source")
                    && scripts.eq_ignore_ascii_case("scripts") =>
            {
                pair_root.join("source/scripts")
            }
            _ => continue,
        };

        if counterpart.is_dir() && !resolved.contains(&counterpart) {
            resolved.push(counterpart);
        }
    }

    resolved
}

/// Returns the existing source directories that will be searched for scripts.
/// Conventional project directories come first, followed by configured roots.
pub fn detected_script_roots(root: &Path, additional_roots: &[String]) -> Vec<PathBuf> {
    CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir))
        .chain(resolve_additional_roots(root, additional_roots))
        .filter(|path| path.is_dir())
        .collect()
}

/// Recursively scans `dir` and every subdirectory beneath it for `.psc`
/// files, matched case-insensitively on extension, and returns their paths
/// in sorted order for a deterministic report.
///
/// Used for the CLI's/desktop app's directory-scan mode (see
/// [`crate::achlist`]'s module docs), which lints every script found under
/// a given directory instead of requiring an `.achlist` — useful for a mod
/// whose scripts are spread across arbitrarily nested subfolders (e.g.
/// Requiem's own layout, see
/// <https://github.com/idrinth/papyrus-lint/issues>) rather than the flat
/// `scripts/source` an `.achlist` conventionally lists.
pub fn find_psc_files_recursively(dir: &Path) -> Vec<PathBuf> {
    let mut results: Vec<PathBuf> = WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            !path.is_dir()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
        })
        .collect();
    results.sort();
    results
}

/// Warns when `script_path` has a same-named, byte-different counterpart in
/// another script search directory.
///
/// Papyrus resolves a script by search-root precedence, so having multiple
/// versions available makes the source used by the compiler dependent on its
/// import-directory ordering. Identical copies are harmless and are ignored.
/// With `short_paths`, the conflicting counterpart's path in the message is
/// shortened relative to `root`, exactly like [`display_path`] shortens
/// every other diagnostic's own reported path.
pub fn conflicting_script_versions(
    script_path: &Path,
    root: &Path,
    additional_roots: &[String],
    short_paths: bool,
    game: Game,
) -> Vec<papyrus_lints::Diagnostic> {
    let Some(file_name) = script_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Some(current) = file_content_hash(script_path, game) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for search_root in detected_script_roots(root, additional_roots) {
        for candidate in dir_children(&search_root) {
            let same_name = candidate
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name));
            if !same_name || !candidate.is_file() || candidate == script_path {
                continue;
            }
            if !paths.contains(&candidate) {
                paths.push(candidate);
            }
        }
    }

    papyrus_lints::conflicting_script_versions::check(
        script_path,
        &current,
        &project_files(paths, root, short_paths, game),
    )
}

/// Maps a script file name (case-insensitively lowercased, as returned by
/// [`detected_script_roots`]'s directories) to every path found under those
/// directories carrying that name. Built once by [`build_script_index`] so
/// both exact-name resolution and conflict checks over a whole batch of
/// scripts (e.g. an achlist's worth) can avoid re-scanning the same roots.
pub type ScriptIndex = HashMap<String, Vec<PathBuf>>;

/// Looks up `name` in a pre-built [`ScriptIndex`], returning the first path
/// in search-root order. `name` may be supplied with or without `.psc` and
/// is matched case-insensitively, just like [`find_psc_file`].
pub fn find_psc_file_in_index(index: &ScriptIndex, name: &str) -> Option<PathBuf> {
    let name_lower = name.to_ascii_lowercase();
    let target = if name_lower.ends_with(".psc") {
        name_lower
    } else {
        format!("{name_lower}.psc")
    };

    index.get(&target).and_then(|paths| paths.first()).cloned()
}

/// Scans `root`'s conventional and configured search directories (see
/// [`detected_script_roots`]) once, recording every `.psc` file found under
/// them by lowercased file name. Reuse the result with
/// [`find_psc_file_in_index`] for name resolution and
/// [`conflicting_script_versions_in_index`] for each script being checked.
pub fn build_script_index(root: &Path, additional_roots: &[String]) -> ScriptIndex {
    index_psc_files(detected_script_roots(root, additional_roots))
}

/// Like [`build_script_index`], but indexes only analysis-only lookup
/// directories (see [`find_psc_file_in_lookup_roots`]). The result is used
/// for name resolution, never for [`conflicting_script_versions_in_index`].
pub fn build_lookup_index(root: &Path, lookup_roots: &[String]) -> ScriptIndex {
    index_psc_files(resolve_additional_roots(root, lookup_roots))
}

/// Process-wide cache of [`build_lookup_index`] results, keyed by the
/// resolved lookup directories and each directory's modification time so a
/// later `FunctionTable` in the same process (the desktop app's per-file
/// lint commands) can skip re-walking a large vanilla `Scripts/Source`
/// tree. Rebuilt when a directory's mtime changes, so a newly added file
/// is still picked up.
type LookupIndexCacheKey = Vec<(PathBuf, u128)>;
type LookupIndexCache = Mutex<HashMap<LookupIndexCacheKey, Arc<ScriptIndex>>>;

static LOOKUP_INDEX_CACHE: OnceLock<LookupIndexCache> = OnceLock::new();

fn lookup_index_cache() -> &'static LookupIndexCache {
    LOOKUP_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn dir_mtime_nanos(path: &Path) -> Option<u128> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_nanos())
}

fn lookup_index_cache_key(root: &Path, lookup_roots: &[String]) -> LookupIndexCacheKey {
    resolve_additional_roots(root, lookup_roots)
        .into_iter()
        .map(|path| {
            let mtime = dir_mtime_nanos(&path).unwrap_or(0);
            (path, mtime)
        })
        .collect()
}

/// Returns [`build_lookup_index`] for `root`/`lookup_roots`, reusing a
/// previous scan of the same directories in this process while their
/// modification times are unchanged.
pub fn cached_lookup_index(root: &Path, lookup_roots: &[String]) -> Arc<ScriptIndex> {
    cached_index(
        lookup_index_cache(),
        lookup_index_cache_key(root, lookup_roots),
        || build_lookup_index(root, lookup_roots),
    )
}

static SCRIPT_INDEX_CACHE: OnceLock<LookupIndexCache> = OnceLock::new();

fn script_index_cache() -> &'static LookupIndexCache {
    SCRIPT_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn script_index_cache_key(root: &Path, additional_roots: &[String]) -> LookupIndexCacheKey {
    detected_script_roots(root, additional_roots)
        .into_iter()
        .map(|path| {
            let mtime = dir_mtime_nanos(&path).unwrap_or(0);
            (path, mtime)
        })
        .collect()
}

/// Returns [`build_script_index`] for `root`/`additional_roots`, reusing a
/// previous scan while those directories' modification times are unchanged.
/// Desktop per-file lint commands use this so each file does not re-walk
/// the project's source trees just to find same-named copies.
pub fn cached_script_index(root: &Path, additional_roots: &[String]) -> Arc<ScriptIndex> {
    cached_index(
        script_index_cache(),
        script_index_cache_key(root, additional_roots),
        || build_script_index(root, additional_roots),
    )
}

fn cached_index(
    cache: &LookupIndexCache,
    key: LookupIndexCacheKey,
    build: impl FnOnce() -> ScriptIndex,
) -> Arc<ScriptIndex> {
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(index) = cache.get(&key) {
        return Arc::clone(index);
    }
    let index = Arc::new(build());
    cache.insert(key, Arc::clone(&index));
    index
}

fn index_psc_files(dirs: impl IntoIterator<Item = PathBuf>) -> ScriptIndex {
    let mut index: ScriptIndex = HashMap::new();

    for search_root in dirs {
        for path in dir_children(&search_root) {
            if !path.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let bucket = index.entry(file_name.to_ascii_lowercase()).or_default();
            if !bucket.contains(&path) {
                bucket.push(path);
            }
        }
    }

    index
}

/// Like [`conflicting_script_versions`], but checks `script_path` against a
/// pre-built [`ScriptIndex`] (see [`build_script_index`]) instead of
/// scanning `root`'s search directories itself. Use this when checking many
/// scripts from the same project in one run, so the directories are only
/// scanned once for the whole batch rather than once per script. `root` and
/// `short_paths` are forwarded to [`conflicting_script_versions_among`].
pub fn conflicting_script_versions_in_index(
    script_path: &Path,
    index: &ScriptIndex,
    root: &Path,
    short_paths: bool,
    game: Game,
) -> Vec<papyrus_lints::Diagnostic> {
    let Some(file_name) = script_path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let Some(candidates) = index.get(&file_name.to_ascii_lowercase()) else {
        return Vec::new();
    };
    if !has_other_candidate(script_path, candidates) {
        return Vec::new();
    }

    conflicting_script_versions_among(script_path, candidates, root, short_paths, game)
}

/// Warns when `script_path` has a same-named, byte-different counterpart
/// among `known_scripts` — other scripts named explicitly (e.g. an
/// `.achlist`'s own entries) rather than discovered by directory search.
///
/// Complements [`conflicting_script_versions`], which only scans `root`'s
/// conventional and configured search directories: two `.achlist` entries
/// can share a file name while living in directories that were never (and,
/// to keep resolution scoped to what was actually listed, deliberately
/// aren't) treated as search roots for one another. `known_scripts` should
/// generally be pre-filtered to only the paths sharing `script_path`'s file
/// name (see [`crate::function_table::FunctionTable::with_known_scripts`]
/// for the matching resolution side of this), so checking every script in a
/// large achlist stays proportional to how many of them actually collide by
/// name rather than to the achlist's full size. With `short_paths`, a
/// conflict's reported path is shortened relative to `root`, exactly like
/// [`conflicting_script_versions`]'s own.
pub fn conflicting_script_versions_among(
    script_path: &Path,
    known_scripts: &[PathBuf],
    root: &Path,
    short_paths: bool,
    game: Game,
) -> Vec<papyrus_lints::Diagnostic> {
    if !has_other_candidate(script_path, known_scripts) {
        return Vec::new();
    }
    let Some(current) = file_content_hash(script_path, game) else {
        return Vec::new();
    };
    papyrus_lints::conflicting_script_versions::check(
        script_path,
        &current,
        &project_files(known_scripts.iter().cloned(), root, short_paths, game),
    )
}

/// Whether `candidates` contains a path other than `script_path`.
fn has_other_candidate(script_path: &Path, candidates: &[PathBuf]) -> bool {
    candidates.iter().any(|candidate| candidate != script_path)
}

#[cfg(test)]
#[path = "script_locator_tests.rs"]
mod tests;

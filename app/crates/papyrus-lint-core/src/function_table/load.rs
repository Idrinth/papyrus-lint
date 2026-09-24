//! Locate, parse, and cache a script on demand for a [`super::FunctionTable`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use super::FunctionTable;
use crate::script_functions::ScriptFunctions;
use crate::script_locator::{find_psc_file, find_psc_file_in_index, find_psc_file_in_lookup_roots};
use crate::source_encoding::read_psc_source;

/// Whether a resolved path came from the project's own search roots (or
/// known-scripts map) or from analysis-only [`FunctionTable::with_lookup_roots`]
/// directories.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptOrigin {
    Project,
    Lookup,
}

/// Process-wide cache of scripts loaded from lookup roots, keyed by path
/// and valid only while that file's mtime is unchanged. Lets a later
/// `FunctionTable` in the same process (desktop per-file lint commands)
/// skip re-reading and re-parsing vanilla game sources already resolved
/// earlier in the session, matching how the on-disk [`crate::ast_cache`]
/// already reuses a previous CLI invocation.
struct LookupScriptEntry {
    mtime: SystemTime,
    script: Option<ScriptFunctions>,
}

fn lookup_script_cache() -> &'static Mutex<HashMap<PathBuf, LookupScriptEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, LookupScriptEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn cached_lookup_script(path: &Path, mtime: SystemTime) -> Option<Option<ScriptFunctions>> {
    let cache = lookup_script_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .get(path)
        .and_then(|entry| (entry.mtime == mtime).then(|| entry.script.clone()))
}

pub(super) fn store_lookup_script(
    path: PathBuf,
    mtime: SystemTime,
    script: Option<ScriptFunctions>,
) {
    let mut cache = lookup_script_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert(path, LookupScriptEntry { mtime, script });
}

/// Process-wide cache of scripts loaded from the bundled vanilla/extender
/// blob by `ScriptName`. Name lookups have no path or mtime, so this is the
/// only reuse across `FunctionTable`s in the same process.
fn bundled_script_cache() -> &'static Mutex<HashMap<String, Option<ScriptFunctions>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<ScriptFunctions>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bundled_script_functions(
    game: papyrus_lint_globals::Game,
    name_lower: &str,
) -> Option<ScriptFunctions> {
    {
        let cache = bundled_script_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = cache.get(name_lower) {
            return cached.clone();
        }
    }
    let loaded = crate::ast_cache::ast_for_script_name(game, name_lower)
        .map(|ast| ScriptFunctions::from_script(&ast, ""));
    let mut cache = bundled_script_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert(name_lower.to_string(), loaded.clone());
    loaded
}

/// Parse `path` through [`crate::ast_cache`], the same path linted source
/// files take via `ast_cache::ensure_primed_for_game`. Bundled vanilla/
/// extender scripts whose content matches `game`'s own
/// `shared/scripts/*-scripts.zip`/`*-extender-scripts.zip` hit the cache's
/// bundled blob and never take its disk lock, so parallel workers
/// resolving the same base type do not serialize on that lookup.
fn load_script_functions(game: papyrus_lint_globals::Game, path: &Path) -> Option<ScriptFunctions> {
    let source = read_psc_source(path).ok()?;
    let parsed = if let Some(cached) = crate::ast_cache::get_for_game(game, path, &source) {
        cached
    } else {
        let parsed = papyrus_parser::parse(&source).ok()?;
        crate::ast_cache::put_for_game(game, path, &source, &parsed);
        if let Ok(tokens) = papyrus_parser::tokenize(&source) {
            crate::ast_cache::put_tokens_for_game(game, path, &source, &tokens);
        }
        parsed
    };
    Some(ScriptFunctions::from_script(&parsed, &source))
}

impl FunctionTable {
    /// Whether a script named `type_name` can be located at all: either
    /// found under the project root (regardless of whether it parses
    /// cleanly), known as a bundled vanilla/extender script (see
    /// [`papyrus_ast_cache::contains_script_name`]), or known as a native
    /// singleton script always called through its literal name (e.g.
    /// `Game`, `Utility`, `Debug`; see [`crate::native_globals`]). In
    /// known-scripts mode (see [`Self::with_known_scripts`]), "found under
    /// the project root" means registered there specifically —
    /// `root`/`additional_roots` are never scanned. Matched
    /// case-insensitively. Used by the "Unresolved script reference" lint
    /// (`papyrus_lints::unresolved_script`) to flag a call like
    /// `MyMissingScript.DoThing()`.
    pub fn script_exists(&self, type_name: &str) -> bool {
        let name_lower = type_name.to_ascii_lowercase();
        self.resolve_script_path(&name_lower).is_some()
            || crate::ast_cache::contains_script_name(self.game, &name_lower)
            || crate::native_globals::is_known_for(self.game, &name_lower)
    }

    fn resolve_script_path(&self, name_lower: &str) -> Option<PathBuf> {
        self.resolve_script_path_kind(name_lower)
            .map(|(path, _)| path)
    }

    pub(super) fn resolve_script_path_kind(
        &self,
        name_lower: &str,
    ) -> Option<(PathBuf, ScriptOrigin)> {
        let primary = match &self.known_scripts {
            Some(known) => known.get(&name_lower.to_ascii_lowercase()).cloned(),
            None => match &self.script_index {
                Some(index) => find_psc_file_in_index(index, name_lower),
                None => find_psc_file(&self.root, name_lower, &self.additional_roots),
            },
        };
        if let Some(path) = primary {
            return Some((path, ScriptOrigin::Project));
        }
        self.resolve_lookup_script_path(name_lower)
            .map(|path| (path, ScriptOrigin::Lookup))
    }

    fn resolve_lookup_script_path(&self, name_lower: &str) -> Option<PathBuf> {
        if let Some(index) = &self.lookup_index {
            return find_psc_file_in_index(index, name_lower);
        }
        if self.lookup_roots.is_empty() {
            return None;
        }
        find_psc_file_in_lookup_roots(&self.root, name_lower, &self.lookup_roots)
    }

    /// Parses and caches the script named `type_name`, if it hasn't been
    /// already. Cache keys are always ASCII-lowercased, so a later lookup
    /// of `Actor` after a walk that saw `Extends Actor` (or `actor`) hits
    /// the same slot instead of parsing twice. In known-scripts mode (see
    /// [`Self::with_known_scripts`]), only an O(1) lookup against the
    /// registered map is ever done against the project's own scripts; a
    /// name not registered there still falls back to
    /// [`Self::with_lookup_roots`] so vanilla game scripts can resolve
    /// without being listed. Otherwise, the lowercased name is looked up
    /// with [`find_psc_file`] as before `with_known_scripts` existed, then
    /// lookup roots. A name still unresolved after that is loaded from the
    /// bundled vanilla/extender AST cache by `ScriptName` for any game with
    /// a bundled blob (Skyrim, Fallout 4), so engine types (`Actor`,
    /// `ObjectReference`, `Form`, …) resolve without game data on disk.
    /// Reuses the on-disk [`crate::ast_cache`] when the script's content
    /// and modification time haven't changed since it was last parsed, so
    /// repeatedly resolving the same cross-script lookup (across separate
    /// CLI invocations, or separate desktop app commands) skips
    /// re-parsing it. Bundled scripts whose content still matches that
    /// game's own `shared/scripts/*-scripts.zip` or
    /// `shared/scripts/*-extender-scripts.zip` hit that crate's bundled
    /// blob instead of the on-disk cache, so the first analysis of a
    /// project does not re-parse `Actor`/`Form`/… either, and parallel
    /// workers resolving those base types do not serialize on the
    /// disk-cache lock. Scripts found only under lookup roots are also kept in
    /// a process-wide table keyed by path+mtime, so a later `FunctionTable` in
    /// this process does not re-read them either.
    pub(super) fn ensure_loaded(&mut self, type_name: &str) {
        let name_lower = type_name.to_ascii_lowercase();
        let resolved = self.resolve_script_path_kind(&name_lower);
        let mtime = resolved.as_ref().and_then(|(path, _)| file_mtime(path));
        if self.scripts.contains_key(&name_lower)
            && self.script_mtimes.get(&name_lower) == Some(&mtime)
        {
            return;
        }

        let script = match resolved {
            Some((path, origin)) => {
                if origin == ScriptOrigin::Lookup {
                    if let Some(mtime) = file_mtime(&path) {
                        if let Some(cached) = cached_lookup_script(&path, mtime) {
                            cached
                        } else {
                            let loaded = load_script_functions(self.game, &path);
                            store_lookup_script(path, mtime, loaded.clone());
                            loaded
                        }
                    } else {
                        load_script_functions(self.game, &path)
                    }
                } else {
                    load_script_functions(self.game, &path)
                }
            }
            None => bundled_script_functions(self.game, &name_lower),
        };

        self.invalidate_descendant_index_if_project_script(&name_lower);
        self.scripts.insert(name_lower.clone(), script);
        self.script_mtimes.insert(name_lower, mtime);
    }

    /// Caches bundled and known-missing names discovered while parsing, so
    /// lint does not take the write lock to rediscover them.
    ///
    /// A name that now resolves to a `.psc` (or, for an "unresolved" name,
    /// to the bundled blob) is left alone: [`Self::preload`] / a later
    /// [`Self::ensure_loaded`] must win, so a same-stem file in a
    /// higher-priority root is never overwritten by a blob or a negative
    /// cache entry.
    pub fn preload_name_slots(&mut self, bundled: &[String], unresolved: &[String]) {
        for name_lower in bundled {
            self.insert_bundled_slot(name_lower);
        }
        for name_lower in unresolved {
            self.insert_unresolved_slot(name_lower);
        }
    }

    fn insert_bundled_slot(&mut self, name_lower: &str) {
        if self.scripts.contains_key(name_lower) {
            return;
        }
        let (resolved_path, mtime) = self.resolved_path_and_mtime(name_lower);
        if resolved_path.is_some() {
            return;
        }
        let script = bundled_script_functions(self.game, name_lower);
        self.invalidate_descendant_index_if_project_script(name_lower);
        self.scripts.insert(name_lower.to_string(), script);
        self.script_mtimes.insert(name_lower.to_string(), mtime);
    }

    fn insert_unresolved_slot(&mut self, name_lower: &str) {
        if self.scripts.contains_key(name_lower) {
            return;
        }
        let (resolved_path, mtime) = self.resolved_path_and_mtime(name_lower);
        if resolved_path.is_some() || crate::ast_cache::contains_script_name(self.game, name_lower)
        {
            return;
        }
        self.invalidate_descendant_index_if_project_script(name_lower);
        self.scripts.insert(name_lower.to_string(), None);
        self.script_mtimes.insert(name_lower.to_string(), mtime);
    }

    /// Resolves `name_lower` to a path and that path's current mtime, the
    /// same read-only lookup [`Self::ensure_loaded`] does before deciding
    /// whether to (re)load it. Used by [`super::PreloadedScript`]'s
    /// [`super::FunctionTable::preload`] to check whether a caller-supplied
    /// path/AST for `name_lower` is still exactly what this table would
    /// resolve on its own, without loading (or locking) anything itself.
    pub(super) fn resolved_path_and_mtime(
        &self,
        name_lower: &str,
    ) -> (Option<PathBuf>, Option<SystemTime>) {
        let resolved = self.resolve_script_path_kind(name_lower);
        let mtime = resolved.as_ref().and_then(|(path, _)| file_mtime(path));
        (resolved.map(|(path, _)| path), mtime)
    }

    /// Cached slot for `name` when it is still valid for the file's current
    /// mtime. `None` means a writer must call [`Self::ensure_loaded`].
    /// `Some(None)` is a cached unresolved type.
    pub(super) fn get_cached(&self, name: &str) -> Option<&Option<ScriptFunctions>> {
        let resolved = self.resolve_script_path_kind(name);
        let mtime = resolved.as_ref().and_then(|(path, _)| file_mtime(path));
        (self.script_mtimes.get(name) == Some(&mtime)).then(|| self.scripts.get(name))?
    }
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;

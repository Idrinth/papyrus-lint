//! Builds a lookup table of function signatures (parameter names and
//! types, plus the return type) for Papyrus object types, by locating and
//! parsing their source scripts on demand.
//!
//! Resolving a call like `SomeObject.SomeFunction(...)` requires knowing
//! the argument names/types and return type declared on `SomeObject`'s
//! script — and, since Papyrus scripts inherit via `Extends`, potentially
//! on any of its ancestors too. [`FunctionTable`] finds and parses those
//! scripts (using [`crate::script_locator`]) at most once per type name
//! and caches the result, so looking up functions while linting many
//! other files stays fast.
//!
//! Converting a parsed script into the cached per-type signature/property/
//! state data lives in [`crate::script_functions`]; this module re-exports
//! its [`FunctionSignature`], [`PropertySignature`] and [`Member`] types.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::script_functions::ScriptFunctions;
pub use crate::script_functions::{FunctionSignature, Member, PropertySignature};

use crate::script_locator::{cached_lookup_index, ScriptIndex};

mod ancestry;
mod closure;
mod external;
mod known;
mod load;
mod peer;
mod shared;

use known::known_script_keys;

pub(crate) use peer::enter_peer_scope;

pub use closure::{ClosedScripts, ParsedDependency, TypeClosureOptions};
pub use shared::SharedFunctionTable;

/// Result of answering a lookup from already-cached scripts, without
/// filling the table. [`Self::Miss`] means a caller that can write should
/// run [`FunctionTable::ensure_loaded`] and retry; [`Self::Hit`] is the
/// complete answer from current cache contents.
pub(super) enum CacheProbe<T> {
    Hit(T),
    Miss,
}

impl<T> CacheProbe<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> CacheProbe<U> {
        match self {
            CacheProbe::Hit(value) => CacheProbe::Hit(f(value)),
            CacheProbe::Miss => CacheProbe::Miss,
        }
    }
}

/// Lazily-populated, cross-file lookup table of function signatures, keyed
/// by object (script) type name.
///
/// Each type name is resolved to a `.psc` file at most once: the file is
/// located from a supplied index or with [`crate::script_locator::find_psc_file`],
/// parsed, and its function signatures (along with its `Extends` parent)
/// are cached. A type that can't be found or fails to parse is cached as
/// unresolved so repeated lookups don't retry the filesystem or parser.
pub struct FunctionTable {
    game: papyrus_lint_globals::Game,
    root: PathBuf,
    additional_roots: Vec<String>,
    /// Analysis-only fallback directories, searched after `root` /
    /// `additional_roots` (and after `known_scripts` in strict-scope mode).
    /// Never used for collision checks.
    lookup_roots: Vec<String>,
    /// When `Some`, resolution is restricted to exactly the scripts
    /// registered here by name (lowercased file stem, path-derived
    /// `folder:stem`, and declared `ScriptName` → path), plus native
    /// singleton globals (see [`Self::script_exists`]/[`Self::ensure_loaded`])
    /// — `root`/`additional_roots` are never scanned at all, so nothing
    /// outside this map can resolve, not even a
    /// same-named file sitting right next to one of these paths, or one
    /// under the conventional `scripts/source`/`source/scripts` layout.
    /// Populated wholesale by [`Self::with_known_scripts`] from an explicit
    /// list of paths (e.g. an `.achlist`'s own entries). `None` (the
    /// default) preserves ordinary directory-based resolution instead.
    /// [`Self::with_lookup_roots`] is still consulted as a last-resort
    /// fallback, so vanilla game scripts can resolve without being listed.
    /// Names still unresolved after that fall through to the bundled
    /// vanilla/SKSE AST cache by `ScriptName`.
    known_scripts: Option<HashMap<String, PathBuf>>,
    /// Snapshot of ordinary directory-based resolution, when the caller has
    /// already scanned the search roots. Unlike `known_scripts`, this does
    /// not change resolution scope; it only avoids repeating directory reads.
    script_index: Option<Arc<ScriptIndex>>,
    /// Snapshot of analysis-only lookup directories (see
    /// [`Self::with_lookup_roots`]), built the same way as `script_index`.
    lookup_index: Option<Arc<ScriptIndex>>,
    /// File stem → paths, for peer lookup only. Not an index key: conflict
    /// checks still use the qualified path in [`ScriptIndex`].
    leaf_paths: HashMap<String, Vec<peer::LeafHit>>,
    scripts: HashMap<String, Option<ScriptFunctions>>,
    /// mtime of the file each `scripts` entry was loaded from,
    /// or `None` when that name was cached as unresolved. Compared on the
    /// next [`Self::ensure_loaded`] so a long-lived table (the desktop
    /// app's process-wide shared table, or a CLI table reused across a
    /// `fix` that rewrote a dependency) picks up an edited `.psc` instead
    /// of serving the previous parse.
    script_mtimes: HashMap<String, Option<SystemTime>>,
    /// Lowercased state names targeted by a literal `GoToState` in some
    /// project script that extends the key (and that ancestor actually
    /// declares the state). `None` until the first
    /// [`Self::descendant_targets_state`] call, and cleared when a project
    /// script is reloaded.
    descendant_goto_targets: Option<HashMap<String, HashSet<String>>>,
    /// Project script names (lowercased file stems) the descendant-target
    /// index was built from. Inserts of other names — bundled vanilla
    /// scripts, unresolved lookups — must not drop that index.
    indexed_project_scripts: Option<HashSet<String>>,
    /// Inherited event names for types whose `Extends` chain fully
    /// resolved, keyed by lowercased type name. [`Self::has_event`] answers
    /// from this set with one lookup after the chain has been walked once,
    /// instead of re-resolving every ancestor on every event. Incomplete
    /// chains are not stored.
    event_index: HashMap<String, ancestry::ResolvedEvents>,
    /// Mtimes of the source directories [`Self::event_index`] was built
    /// against. `None` until the first event query. A change means a
    /// script appeared or disappeared and the index is stale.
    event_index_roots: Option<Vec<Option<SystemTime>>>,
}

impl FunctionTable {
    /// Project root searched by this table.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Additional search roots, in resolution order.
    pub fn additional_roots(&self) -> &[String] {
        &self.additional_roots
    }

    /// Creates an empty table that resolves script names against
    /// `scripts/source` / `source/scripts` under `root`.
    pub fn new(root: PathBuf) -> Self {
        FunctionTable {
            game: papyrus_lint_globals::Game::default(),
            root,
            additional_roots: Vec::new(),
            lookup_roots: Vec::new(),
            known_scripts: None,
            script_index: None,
            lookup_index: None,
            leaf_paths: HashMap::new(),
            scripts: HashMap::new(),
            script_mtimes: HashMap::new(),
            descendant_goto_targets: None,
            indexed_project_scripts: None,
            event_index: HashMap::new(),
            event_index_roots: None,
        }
    }

    /// Creates an empty table that also searches `additional_roots` (see
    /// [`papyrus_lint_config::load_script_roots`]/[`crate::script_locator::find_psc_file`])
    /// alongside `scripts/source` / `source/scripts` under `root`.
    pub fn new_with_additional_roots(root: PathBuf, additional_roots: Vec<String>) -> Self {
        FunctionTable {
            game: papyrus_lint_globals::Game::default(),
            root,
            additional_roots,
            lookup_roots: Vec::new(),
            known_scripts: None,
            script_index: None,
            lookup_index: None,
            leaf_paths: HashMap::new(),
            scripts: HashMap::new(),
            script_mtimes: HashMap::new(),
            descendant_goto_targets: None,
            indexed_project_scripts: None,
            event_index: HashMap::new(),
            event_index_roots: None,
        }
    }

    /// Selects the target game used to namespace on-disk AST cache entries.
    pub fn with_game(mut self, game: papyrus_lints::Game) -> Self {
        game.assert_supported();
        self.game = game;
        self
    }

    /// Analysis-only fallback directories, searched after the project's own
    /// roots (and after `known_scripts` in strict-scope mode). Scripts found
    /// only here are never linted and are never considered by
    /// `conflicting_script_versions`.
    pub fn with_lookup_roots(mut self, lookup_roots: Vec<String>) -> Self {
        if !lookup_roots.is_empty() {
            // Walked once per unique directory fingerprint and reused for
            // later tables (desktop per-file commands, subsequent lookups
            // in the same process), the same way `with_script_index` reuses
            // a scan of the project's own source directories.
            self.lookup_index = Some(cached_lookup_index(&self.root, &lookup_roots));
        }
        self.lookup_roots = lookup_roots;
        self.rebuild_leaf_paths();
        self
    }

    /// Switches this table into known-scripts mode, where only `paths` (by
    /// file stem, path-derived `folder:stem`, and declared `ScriptName`,
    /// each matched case-insensitively) and native singleton globals
    /// can resolve at all — `root`/`additional_roots` are never scanned
    /// again for the rest of this table's lifetime, in [`Self::ensure_loaded`]/
    /// [`Self::script_exists`] alike. Intended for a set of scripts named
    /// explicitly rather than discovered by directory search — e.g. an
    /// `.achlist`'s own entries, which may live in arbitrary directories
    /// outside `scripts/source`/`source/scripts` and not share a directory
    /// with each other at all: cross-script resolution among them still
    /// works, without treating their parent directories as search roots
    /// (which would also expose every other file in them, including one
    /// under the conventional `scripts/source`/`source/scripts` layout).
    /// [`Self::with_lookup_roots`] is still consulted afterwards, so a
    /// vanilla game script can resolve without being listed. When two given
    /// paths share a file stem, the first one wins for that stem, matching
    /// a directory search's own first-match-wins order; qualified names
    /// (`User:Foo` vs `Other:Foo`) stay distinct slots. A real conflict
    /// between such paths is instead reported by
    /// [`crate::script_locator::conflicting_script_versions_among`].
    pub fn with_known_scripts(mut self, paths: &[PathBuf]) -> Self {
        let mut known = HashMap::new();
        for path in paths {
            for key in known_script_keys(&self.root, &self.additional_roots, path) {
                known.entry(key).or_insert_with(|| path.clone());
            }
        }
        self.known_scripts = Some(known);
        self
    }

    /// Keys under which a preload of `path` is stored. Includes `explicit`
    /// (the name the caller already resolved) plus the file stem, the
    /// path-derived `folder:stem`, and the declared `ScriptName`, but only
    /// when this table would load `path` for that key. A nested
    /// `Scripts/Source/User/Foo.psc` is therefore cached as `user:foo`. It
    /// is not cached as a bare `foo` unless a flat file of that stem is
    /// what resolution returns — and only for the path that wins.
    pub(super) fn cache_keys_resolving_to(&self, path: &Path, explicit: &str) -> Vec<String> {
        let mut keys = Vec::new();
        let mut consider = |raw: &str| {
            let key = raw.to_ascii_lowercase();
            if key.is_empty() || keys.contains(&key) {
                return;
            }
            let resolves_here = self
                .resolve_script_path_kind(&key)
                .is_some_and(|(resolved, _)| resolved.as_path() == path);
            if resolves_here {
                keys.push(key);
            }
        };
        consider(explicit);
        for key in known_script_keys(&self.root, &self.additional_roots, path) {
            consider(&key);
        }
        keys
    }

    /// Reuses a snapshot of the table's normal search directories for O(1)
    /// name lookup while preserving their first-match-wins resolution order.
    pub fn with_script_index(mut self, index: Arc<ScriptIndex>) -> Self {
        self.script_index = Some(index);
        self.rebuild_leaf_paths();
        self
    }

    fn rebuild_leaf_paths(&mut self) {
        let mut leaves = HashMap::new();
        if let Some(index) = &self.script_index {
            peer::push_index_leaves(index, load::ScriptOrigin::Project, &mut leaves);
        }
        if let Some(index) = &self.lookup_index {
            peer::push_index_leaves(index, load::ScriptOrigin::Lookup, &mut leaves);
        }
        self.leaf_paths = leaves;
    }

    /// Merges `entries` -- typically every script a run is about to lint,
    /// already read and parsed by the caller in its own "parse every file
    /// first" pass, before this table is shared read-write across lint
    /// workers (see [`SharedFunctionTable`]) -- into this table's cache in
    /// one pass, without ever taking a lock: each entry's resolution is
    /// double-checked against what this table would resolve `name_lower` to
    /// on its own ([`FunctionTable::resolved_path_and_mtime`]), and only
    /// substituted in when the two agree, so a name that actually resolves
    /// elsewhere (e.g. a same-named script in a higher-priority search root
    /// that isn't part of `entries`) is left for [`Self::ensure_loaded`] to
    /// resolve correctly later, exactly as it would without this call. A
    /// name two entries both claim keeps the first (matching directory
    /// search's own first-match-wins order).
    ///
    /// The caller's `name_lower` is not the only slot. The same parsed
    /// script is also stored under every other key that resolves to that
    /// path — file stem when it wins, path-derived `folder:stem`
    /// (`user:foo`), and declared `ScriptName` — so `Extends User:Foo` and
    /// `script_exists("User:Foo")` hit the cache instead of the stem alone.
    /// A key that resolves to a different file is left untouched.
    ///
    /// Once this returns, [`SharedFunctionTable`]'s exclusive write lock is
    /// only ever needed afterward for a name never passed here and never
    /// reached by [`Self::parse_type_closure`] — typically a typo, a dynamic
    /// name, or a type introduced when `--fix` rewrote a file.
    pub fn preload(&mut self, entries: Vec<PreloadedScript<'_>>) {
        for entry in entries {
            let keys = self.cache_keys_resolving_to(entry.path, &entry.name_lower);
            if keys.is_empty() {
                continue;
            }
            let mtime = load::file_mtime(entry.path);
            let functions = entry
                .ast
                .map(|ast| ScriptFunctions::from_script(ast, entry.source));
            let mut stored_lookup = false;
            for key in keys {
                if self.scripts.contains_key(&key) {
                    continue;
                }
                let Some((resolved_path, origin)) = self.resolve_script_path_kind(&key) else {
                    continue;
                };
                if resolved_path.as_path() != entry.path {
                    continue;
                }
                if origin == load::ScriptOrigin::Lookup && !stored_lookup {
                    if let Some(mtime) = mtime {
                        load::store_lookup_script(resolved_path, mtime, functions.clone());
                    }
                    stored_lookup = true;
                }
                self.invalidate_descendant_index_if_project_script(&key);
                self.scripts.insert(key.clone(), functions.clone());
                self.script_mtimes.insert(key, mtime);
            }
        }
    }

    /// Drops the descendant `GoToState` index when a project script is
    /// (re)loaded. Bundled and unresolved names are not in
    /// [`Self::indexed_project_scripts`], so filling those slots during an
    /// ancestry walk does not force the index to be rebuilt per file.
    fn invalidate_descendant_index_if_project_script(&mut self, name_lower: &str) {
        if self
            .indexed_project_scripts
            .as_ref()
            .is_some_and(|names| names.contains(name_lower))
        {
            self.descendant_goto_targets = None;
        }
    }
}

/// One project script already read and parsed by the caller, ready to be
/// merged into a [`FunctionTable`]'s cache by [`FunctionTable::preload`]
/// instead of being resolved and parsed again on first use.
pub struct PreloadedScript<'a> {
    /// The exact path this script was read from -- compared against
    /// [`FunctionTable`]'s own resolution of `name_lower` before its parsed
    /// data is trusted (see [`FunctionTable::preload`]).
    pub path: &'a Path,
    /// Lowercased name the caller resolved this path under. [`FunctionTable::preload`]
    /// also stores the script under every other key that resolves to `path`
    /// (file stem when it wins, path-derived `folder:stem`, declared
    /// `ScriptName`), so `User:Foo` and a flat `Foo` stay distinct slots.
    pub name_lower: String,
    /// This script's parsed AST, or `None` if it failed to parse (still
    /// worth preloading as a cached-unresolved entry, matching what
    /// [`FunctionTable::ensure_loaded`] would cache for an unparseable
    /// script).
    pub ast: Option<&'a papyrus_parser::ast::Script>,
    pub source: &'a str,
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

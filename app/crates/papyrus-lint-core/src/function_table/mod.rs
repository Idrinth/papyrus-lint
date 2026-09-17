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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::script_functions::ScriptFunctions;
pub use crate::script_functions::{FunctionSignature, Member, PropertySignature};

use crate::script_locator::{build_lookup_index, ScriptIndex};

mod ancestry;
mod external;
mod load;
mod shared;

pub use shared::SharedFunctionTable;

/// Lazily-populated, cross-file lookup table of function signatures, keyed
/// by object (script) type name.
///
/// Each type name is resolved to a `.psc` file at most once: the file is
/// located from a supplied index or with [`crate::script_locator::find_psc_file`],
/// parsed, and its function signatures (along with its `Extends` parent)
/// are cached. A type that can't be found or fails to parse is cached as
/// unresolved so repeated lookups don't retry the filesystem or parser.
pub struct FunctionTable {
    root: PathBuf,
    additional_roots: Vec<String>,
    /// Analysis-only fallback directories, searched after `root` /
    /// `additional_roots` (and after `known_scripts` in strict-scope mode).
    /// Never used for collision checks.
    lookup_roots: Vec<String>,
    /// When `Some`, resolution is restricted to exactly the scripts
    /// registered here by name (lowercased file stem -> path), plus native
    /// singleton globals (see [`Self::script_exists`]/[`Self::ensure_loaded`])
    /// — `root`/`additional_roots` are never scanned at all, so nothing
    /// outside this map (or the native fallback) can resolve, not even a
    /// same-named file sitting right next to one of these paths, or one
    /// under the conventional `scripts/source`/`source/scripts` layout.
    /// Populated wholesale by [`Self::with_known_scripts`] from an explicit
    /// list of paths (e.g. an `.achlist`'s own entries). `None` (the
    /// default) preserves ordinary directory-based resolution instead.
    /// [`Self::with_lookup_roots`] is still consulted as a last-resort
    /// fallback, so vanilla game scripts can resolve without being listed.
    known_scripts: Option<HashMap<String, PathBuf>>,
    /// Snapshot of ordinary directory-based resolution, when the caller has
    /// already scanned the search roots. Unlike `known_scripts`, this does
    /// not change resolution scope; it only avoids repeating directory reads.
    script_index: Option<Arc<ScriptIndex>>,
    /// Snapshot of analysis-only lookup directories (see
    /// [`Self::with_lookup_roots`]), built the same way as `script_index`.
    lookup_index: Option<Arc<ScriptIndex>>,
    scripts: HashMap<String, Option<ScriptFunctions>>,
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
            root,
            additional_roots: Vec::new(),
            lookup_roots: Vec::new(),
            known_scripts: None,
            script_index: None,
            lookup_index: None,
            scripts: HashMap::new(),
        }
    }

    /// Creates an empty table that also searches `additional_roots` (see
    /// [`papyrus_lint_config::load_script_roots`]/[`crate::script_locator::find_psc_file`])
    /// alongside `scripts/source` / `source/scripts` under `root`.
    pub fn new_with_additional_roots(root: PathBuf, additional_roots: Vec<String>) -> Self {
        FunctionTable {
            root,
            additional_roots,
            lookup_roots: Vec::new(),
            known_scripts: None,
            script_index: None,
            lookup_index: None,
            scripts: HashMap::new(),
        }
    }

    /// Analysis-only fallback directories, searched after the project's own
    /// roots (and after `known_scripts` in strict-scope mode). Scripts found
    /// only here are never linted and are never considered by
    /// `conflicting_script_versions`.
    pub fn with_lookup_roots(mut self, lookup_roots: Vec<String>) -> Self {
        if !lookup_roots.is_empty() {
            self.lookup_index = Some(Arc::new(build_lookup_index(&self.root, &lookup_roots)));
        }
        self.lookup_roots = lookup_roots;
        self
    }

    /// Switches this table into known-scripts mode, where only `paths` (by
    /// file stem, matched case-insensitively) and native singleton globals
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
    /// vanilla game script can resolve without being listed. When two given paths share a file stem, the first one wins, matching
    /// a directory search's own first-match-wins order; a real conflict
    /// between such paths is instead reported by
    /// [`crate::script_locator::conflicting_script_versions_among`].
    pub fn with_known_scripts(mut self, paths: &[PathBuf]) -> Self {
        let mut known = HashMap::new();
        for path in paths {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            known
                .entry(stem.to_ascii_lowercase())
                .or_insert_with(|| path.clone());
        }
        self.known_scripts = Some(known);
        self
    }

    /// Reuses a snapshot of the table's normal search directories for O(1)
    /// name lookup while preserving their first-match-wins resolution order.
    pub fn with_script_index(mut self, index: Arc<ScriptIndex>) -> Self {
        self.script_index = Some(index);
        self
    }
}

#[cfg(test)]
mod tests;

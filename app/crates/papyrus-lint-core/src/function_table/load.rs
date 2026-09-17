//! Locate, parse, and cache a script on demand for a [`super::FunctionTable`].

use std::path::PathBuf;

use super::FunctionTable;
use crate::script_functions::ScriptFunctions;
use crate::script_locator::{find_psc_file, find_psc_file_in_index, find_psc_file_in_lookup_roots};
use crate::source_encoding::read_psc_source;

impl FunctionTable {
    /// Whether a script named `type_name` can be located at all: either
    /// found under the project root (regardless of whether it parses
    /// cleanly), or known as a native singleton script always called
    /// through its literal name (e.g. `Game`, `Utility`, `Debug`; see
    /// [`crate::native_globals`]). In known-scripts mode (see
    /// [`Self::with_known_scripts`]), "found under the project root" means
    /// registered there specifically — `root`/`additional_roots` are never
    /// scanned. Matched case-insensitively. Used by the "Unresolved script
    /// reference" lint (`papyrus_lints::unresolved_script`) to flag a call
    /// like `MyMissingScript.DoThing()`.
    pub fn script_exists(&mut self, type_name: &str) -> bool {
        let name_lower = type_name.to_ascii_lowercase();
        self.resolve_script_path(&name_lower).is_some()
            || crate::native_globals::is_known(&name_lower)
    }

    fn resolve_script_path(&self, name_lower: &str) -> Option<PathBuf> {
        let primary = match &self.known_scripts {
            Some(known) => known.get(&name_lower.to_ascii_lowercase()).cloned(),
            None => match &self.script_index {
                Some(index) => find_psc_file_in_index(index, name_lower),
                None => find_psc_file(&self.root, name_lower, &self.additional_roots),
            },
        };
        primary.or_else(|| self.resolve_lookup_script_path(name_lower))
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

    /// Parses and caches the script named `name_lower`, if it hasn't been
    /// already. In known-scripts mode (see [`Self::with_known_scripts`]),
    /// only an O(1) lookup against the registered map is ever done against
    /// the project's own scripts; a name not registered there still falls
    /// back to [`Self::with_lookup_roots`] so vanilla game scripts can
    /// resolve without being listed. Otherwise, `name_lower` is looked up
    /// with [`find_psc_file`] as before `with_known_scripts` existed, then
    /// lookup roots. Reuses the on-disk [`crate::ast_cache`] when the
    /// script's content and modification time haven't changed since it was
    /// last parsed, so repeatedly resolving the same cross-script lookup
    /// (across separate CLI invocations, or separate desktop app commands)
    /// skips re-parsing it.
    pub(super) fn ensure_loaded(&mut self, name_lower: &str) {
        if self.scripts.contains_key(name_lower) {
            return;
        }

        // `name_lower` is only actually lowercased on a lookup's initial
        // call; walking further up an `Extends` chain re-enters this with
        // the parent's name cased exactly as written in `Extends ParentName`
        // (see e.g. `lookup_function`'s loop). `find_psc_file` tolerates
        // that by lowercasing internally before matching a directory entry,
        // so the known-scripts map (keyed by an already-lowercased stem)
        // has to do the same explicitly here.
        let resolved_path = self.resolve_script_path(name_lower);

        let script = resolved_path.and_then(|path| {
            let source = read_psc_source(&path).ok()?;
            let parsed = if let Some(cached) = crate::ast_cache::get(&path, &source) {
                cached
            } else {
                let parsed = papyrus_parser::parse(&source).ok()?;
                crate::ast_cache::put(&path, &source, &parsed);
                if let Ok(tokens) = papyrus_parser::tokenize(&source) {
                    crate::ast_cache::put_tokens(&path, &source, &tokens);
                }
                parsed
            };
            Some(ScriptFunctions::from_script(&parsed, &source))
        });

        self.scripts.insert(name_lower.to_string(), script);
    }
}

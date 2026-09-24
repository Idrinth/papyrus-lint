//! Event lookup and inherited-event index maintenance.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;

use super::{parent_cache_key, FunctionTable};

/// One fully resolved type in [`FunctionTable`]'s event index: every event
/// name visible on that type (its own plus ancestors'), and the files that
/// answer was built from.
pub(in crate::function_table) struct ResolvedEvents {
    pub(super) names: HashSet<String>,
    /// `(path, mtime)` from this type up to the root. `path == None` is a
    /// bundled script with no file to stat.
    chain: Vec<(Option<PathBuf>, Option<SystemTime>)>,
}

struct EventStep {
    name: String,
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    own: HashSet<String>,
}

pub(super) fn event_chain_fresh(entry: &ResolvedEvents) -> bool {
    entry.chain.iter().all(|(path, mtime)| match path {
        Some(path) => super::super::load::file_mtime(path) == *mtime,
        None => mtime.is_none(),
    })
}

impl FunctionTable {
    /// Whether a fully resolved `Extends` chain declares `event_name` as an
    /// event. A same-named ordinary function does not stop the search, and
    /// an event that exists only inside a `State` still counts.
    ///
    /// The first query for a type walks the chain once and stores the
    /// result on [`Self`]'s event index. Later queries for that type, or
    /// for a type whose ancestor was already indexed, are a set lookup —
    /// the same shape as [`Self::lookup_function`] stopping on the
    /// declaring script — and do not re-resolve search roots. `None` (the
    /// chain is circular or a script is missing) is not indexed, so a
    /// script that appears later is still found.
    pub fn has_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        let type_key = type_name.to_ascii_lowercase();
        let event_key = event_name.to_ascii_lowercase();
        if !self.build_event_index(&type_key) {
            return None;
        }
        Some(
            self.event_index
                .get(&type_key)
                .is_some_and(|entry| entry.names.contains(&event_key)),
        )
    }

    /// Mtimes of directories a live (non-indexed) name search would walk.
    /// A snapshot [`Self::script_index`] / lookup index is not included:
    /// those already ignore files added after the snapshot, matching
    /// [`Self::ensure_loaded`].
    fn events_root_stamp(&self) -> Vec<Option<SystemTime>> {
        let mut dirs = Vec::new();
        if self.script_index.is_none() && self.known_scripts.is_none() {
            dirs.extend(crate::script_locator::detected_script_roots(
                &self.root,
                &self.additional_roots,
            ));
        }
        if self.lookup_index.is_none() {
            dirs.extend(crate::script_locator::resolve_additional_roots(
                &self.root,
                &self.lookup_roots,
            ));
        }
        dirs.iter()
            .map(|dir| super::super::load::file_mtime(dir))
            .collect()
    }

    pub(super) fn event_roots_match(&self) -> bool {
        self.event_index_roots
            .as_ref()
            .is_some_and(|cached| cached == &self.events_root_stamp())
    }

    /// Fills [`Self::event_index`] for `type_key` and every ancestor.
    /// `false` when the chain cannot be fully resolved.
    fn build_event_index(&mut self, type_key: &str) -> bool {
        if !self.event_roots_match() {
            self.event_index.clear();
            self.event_index_roots = Some(self.events_root_stamp());
        } else if self
            .event_index
            .get(type_key)
            .is_some_and(event_chain_fresh)
        {
            return true;
        }

        let mut steps = Vec::new();
        let mut current = Some(type_key.to_string());
        while let Some(name) = current {
            if steps.iter().any(|step: &EventStep| step.name == name) {
                return false;
            }
            let cached = self
                .event_index
                .get(&name)
                .filter(|existing| event_chain_fresh(existing))
                .map(|existing| (existing.names.clone(), existing.chain.clone()));
            if let Some((names, chain)) = cached {
                self.graft_event_steps(steps, names, chain);
                return true;
            }
            self.ensure_loaded(&name);
            let (path, mtime) = self.resolved_path_and_mtime(&name);
            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                return false;
            };
            let parent = parent_cache_key(script);
            steps.push(EventStep {
                name,
                path,
                mtime,
                own: script.events.clone(),
            });
            current = parent;
        }
        self.store_event_steps(&steps);
        true
    }

    fn store_event_steps(&mut self, steps: &[EventStep]) {
        let mut names = HashSet::new();
        for index in (0..steps.len()).rev() {
            names.extend(steps[index].own.iter().cloned());
            let chain = steps[index..]
                .iter()
                .map(|step| (step.path.clone(), step.mtime))
                .collect();
            self.event_index.insert(
                steps[index].name.clone(),
                ResolvedEvents {
                    names: names.clone(),
                    chain,
                },
            );
        }
    }

    /// `steps` are the types below an already-indexed ancestor, nearest
    /// ancestor last. `names` / `chain` are that ancestor's resolved events.
    fn graft_event_steps(
        &mut self,
        steps: Vec<EventStep>,
        mut names: HashSet<String>,
        mut chain: Vec<(Option<PathBuf>, Option<SystemTime>)>,
    ) {
        for step in steps.into_iter().rev() {
            names.extend(step.own.iter().cloned());
            chain.insert(0, (step.path.clone(), step.mtime));
            self.event_index.insert(
                step.name.clone(),
                ResolvedEvents {
                    names: names.clone(),
                    chain: chain.clone(),
                },
            );
        }
    }
}

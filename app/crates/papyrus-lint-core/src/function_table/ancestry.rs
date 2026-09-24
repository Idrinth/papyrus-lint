//! `Extends`-chain lookups against a [`super::FunctionTable`]: functions,
//! properties, states, members, events, and subtype/ancestry queries.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use super::{CacheProbe, FunctionTable};
use crate::script_functions::{FunctionSignature, Member, ScriptFunctions};
use papyrus_lints::MemberAccess;

fn cached_script<'a>(
    table: &'a FunctionTable,
    name: &str,
) -> CacheProbe<Option<&'a ScriptFunctions>> {
    match table.get_cached(name) {
        None => CacheProbe::Miss,
        Some(slot) => CacheProbe::Hit(slot.as_ref()),
    }
}

/// ASCII-lowercased `Extends` parent, matching [`FunctionTable::ensure_loaded`]'s
/// cache keys. Walks that used the declared casing as the next `current`
/// name would miss a previously loaded lowercase slot (or insert a second
/// one) whenever `Extends Actor` and a later `actor` lookup mixed.
fn parent_cache_key(script: &ScriptFunctions) -> Option<String> {
    script
        .extends
        .as_ref()
        .map(|parent| parent.to_ascii_lowercase())
}

/// One fully resolved type in [`FunctionTable`]'s event index: every event
/// name visible on that type (its own plus ancestors'), and the files that
/// answer was built from.
pub(super) struct ResolvedEvents {
    names: HashSet<String>,
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

fn event_chain_fresh(entry: &ResolvedEvents) -> bool {
    entry.chain.iter().all(|(path, mtime)| match path {
        Some(path) => super::load::file_mtime(path) == *mtime,
        None => mtime.is_none(),
    })
}

impl FunctionTable {
    pub fn function_access(&mut self, type_name: &str, member_name: &str) -> Option<MemberAccess> {
        self.member_access(type_name, member_name, |script, key| {
            script.functions.get(key).map(|member| member.access_level)
        })
    }

    pub fn property_access(&mut self, type_name: &str, member_name: &str) -> Option<MemberAccess> {
        self.member_access(type_name, member_name, |script, key| {
            script.properties.get(key).map(|member| member.access_level)
        })
    }

    fn member_access(
        &mut self,
        type_name: &str,
        member_name: &str,
        find: impl Fn(&ScriptFunctions, &str) -> Option<papyrus_parser::ast::AccessLevel>,
    ) -> Option<MemberAccess> {
        let key = member_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            self.ensure_loaded(&name);
            let script = self.scripts.get(&name)?.as_ref()?;
            if let Some(access_level) = find(script, &key) {
                return Some(MemberAccess {
                    declaring_type: name,
                    access_level,
                });
            }
            current = parent_cache_key(script);
            visited.push(name);
        }
        None
    }

    /// [`Self::member_access`] answered only from scripts already cached.
    /// [`CacheProbe::Miss`] means some ancestor still needs
    /// [`Self::ensure_loaded`].
    pub(super) fn member_access_cached(
        &self,
        type_name: &str,
        member_name: &str,
        find: impl Fn(&ScriptFunctions, &str) -> Option<papyrus_parser::ast::AccessLevel>,
    ) -> CacheProbe<Option<MemberAccess>> {
        let key = member_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(None);
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(None),
                CacheProbe::Hit(Some(script)) => script,
            };
            if let Some(access_level) = find(script, &key) {
                return CacheProbe::Hit(Some(MemberAccess {
                    declaring_type: name,
                    access_level,
                }));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }
        CacheProbe::Hit(None)
    }

    /// Looks up the signature of `function_name` as callable on an object
    /// of type `type_name`, searching `type_name` and its ancestors in
    /// `Extends` order.
    ///
    /// Returns `None` if neither `type_name` nor any ancestor declares a
    /// matching function, or if `type_name`'s script can't be found or
    /// parsed. Both names are matched case-insensitively.
    pub fn lookup_function(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<FunctionSignature> {
        let function_key = function_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let script = self.scripts.get(&name)?.as_ref()?;
            if let Some(signature) = script.functions.get(&function_key) {
                return Some(signature.clone());
            }

            current = parent_cache_key(script);
            visited.push(name);
        }

        None
    }

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
            .map(|dir| super::load::file_mtime(dir))
            .collect()
    }

    fn event_roots_match(&self) -> bool {
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

    /// Whether `sub_type`'s script is, or extends (directly or
    /// transitively), `super_type`. Both names are matched
    /// case-insensitively. When a type along the way isn't a script in the
    /// configured script roots, falls back to the bundled vanilla/SKSE
    /// AST cache by `ScriptName`. Returns `false` once a script in the
    /// chain cannot be resolved before reaching `super_type`.
    pub fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        let super_lower = super_type.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(sub_type.to_ascii_lowercase());

        while let Some(name) = current {
            if name == super_lower {
                return true;
            }
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            current = self
                .scripts
                .get(&name)
                .and_then(Option::as_ref)
                .and_then(parent_cache_key);
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s full `Extends` ancestry resolves all the way to
    /// a definite root: a script found (and parsed) with no `Extends` line
    /// at all. Matched case-insensitively, mirroring
    /// [`Self::is_subtype`]'s own walk (including the bundled vanilla/SKSE
    /// name fallback), but tracking whether
    /// the walk actually reached a confirmed root rather than just whether
    /// it reached a particular type. A circular `Extends` chain, or a type
    /// along the way this table has no data for at all (not a project
    /// script, not in the bundled cache) means the ancestry is *not*
    /// fully known. Used by the "Impossible cast" lint
    /// (`papyrus_lints::impossible_cast`) to tell a value/target pair
    /// *proven* unrelated (both sides fully resolved, per
    /// [`papyrus_lints::ExternalSignatures::ancestry_fully_known`])
    /// apart from one this table simply doesn't have enough information
    /// about.
    pub fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return false; // circular Extends chain; never confidently resolved
            }
            self.ensure_loaded(&name);

            current = match self.scripts.get(&name).and_then(Option::as_ref) {
                Some(script) => match parent_cache_key(script) {
                    Some(parent) => Some(parent),
                    None => return true, // an explicit script with no Extends is a definite root
                },
                None => return false,
            };
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a property named `property_name`. Both
    /// names are matched case-insensitively. Returns `false` if
    /// `type_name`'s script (or any ancestor along the way) can't be found
    /// or parsed before a match is found.
    pub fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        let property_key = property_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.properties.contains_key(&property_key) {
                return true;
            }

            current = parent_cache_key(script);
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a script-level variable (a plain field,
    /// not a `Property`) named `field_name`. Both names are matched
    /// case-insensitively. Returns `false` if `type_name`'s script (or any
    /// ancestor along the way) can't be found or parsed before a match is
    /// found. Used by the "Local variable shadowing" lint
    /// (`papyrus_lints::local_variable_shadowing`) to check a local
    /// variable against a parent script's fields, mirroring
    /// [`Self::has_property`] above.
    pub fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        let field_key = field_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.variables.contains(&field_key) {
                return true;
            }

            current = parent_cache_key(script);
            visited.push(name);
        }

        false
    }

    /// Whether `type_name`'s script, or an ancestor it `Extends` (directly
    /// or transitively), declares a `State` block named `state_name`. Both
    /// names are matched case-insensitively. Returns `false` if
    /// `type_name`'s script (or any ancestor along the way) can't be found
    /// or parsed before a match is found. Used by the "GoToState state
    /// reference" lint (`papyrus_lints::goto_state`) to flag a
    /// `GoToState("Name")` call whose target state can't be resolved
    /// anywhere in the script's own ancestry.
    pub fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        let state_key = state_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            if script.states.contains_key(&state_key) {
                return true;
            }

            current = parent_cache_key(script);
            visited.push(name);
        }

        false
    }

    /// Every named `State` declared anywhere in `type_name`'s own
    /// `Extends` ancestry — `type_name`'s own script, then each further
    /// ancestor it extends — as `(name, is_auto)` pairs. Both a script's
    /// own casing (not lowercased) and every declaration it makes are
    /// included; a script (or an ancestor along the way) that can't be
    /// found or parsed simply ends the walk there rather than failing the
    /// whole lookup, mirroring [`Self::has_state`]. Used by the "Total
    /// named state count"/"Multiple Auto states" lint pair
    /// (`papyrus_lints::state_count`) to tally a script's full inheritance
    /// chain against the engine's per-script limits.
    pub fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            result.extend(
                script
                    .states
                    .iter()
                    .map(|(name, &is_auto)| (name.clone(), is_auto)),
            );

            current = parent_cache_key(script);
            visited.push(name);
        }

        result
    }

    /// Whether a project script that extends `type_name` (directly or
    /// transitively) contains a literal `GoToState` / `self.GoToState`
    /// targeting `state_name`, and `type_name` itself declares that state.
    /// A child `GoToState` activates an ancestor's state, so the unused-state
    /// lint must not flag it. Both names are matched case-insensitively.
    /// Scripts outside the project (bundled vanilla/SKSE scripts, lookup
    /// roots) are not treated as descendants.
    pub fn descendant_targets_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.ensure_descendant_goto_targets();
        self.descendant_goto_targets
            .as_ref()
            .and_then(|index| index.get(&type_name.to_ascii_lowercase()))
            .is_some_and(|targets| targets.contains(&state_name.to_ascii_lowercase()))
    }

    fn ensure_descendant_goto_targets(&mut self) {
        if self.descendant_goto_targets.is_some() {
            return;
        }
        if self.indexed_project_scripts.is_none() {
            self.indexed_project_scripts = Some(self.project_script_names());
        }
        let project_names: Vec<String> = self
            .indexed_project_scripts
            .as_ref()
            .map(|names| names.iter().cloned().collect())
            .unwrap_or_default();
        for name in &project_names {
            self.ensure_loaded(name);
        }
        let mut pending = project_names.clone();
        let mut seen = HashSet::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            self.ensure_loaded(&name);
            if let Some(parent) = self
                .scripts
                .get(&name)
                .and_then(|slot| slot.as_ref())
                .and_then(parent_cache_key)
            {
                pending.push(parent);
            }
        }

        let mut index: HashMap<String, HashSet<String>> = HashMap::new();
        for name in project_names {
            let Some(slot) = self.scripts.get(&name).and_then(|slot| slot.as_ref()) else {
                continue;
            };
            if slot.goto_state_targets.is_empty() {
                continue;
            }
            let targets = slot.goto_state_targets.clone();
            let mut current = parent_cache_key(slot);
            let mut visited = vec![name];
            while let Some(parent) = current {
                if visited.iter().any(|seen| seen == &parent) {
                    break;
                }
                if let Some(script) = self.scripts.get(&parent).and_then(|slot| slot.as_ref()) {
                    let declared: HashSet<String> = targets
                        .iter()
                        .filter(|target| script.states.contains_key(target.as_str()))
                        .cloned()
                        .collect();
                    if !declared.is_empty() {
                        index.entry(parent.clone()).or_default().extend(declared);
                    }
                    current = parent_cache_key(script);
                } else {
                    break;
                }
                visited.push(parent);
            }
        }
        self.descendant_goto_targets = Some(index);
    }

    fn project_script_names(&self) -> HashSet<String> {
        if let Some(known) = &self.known_scripts {
            return known.keys().cloned().collect();
        }
        let index = self.script_index.clone().unwrap_or_else(|| {
            std::sync::Arc::new(crate::script_locator::build_script_index(
                &self.root,
                &self.additional_roots,
            ))
        });
        index
            .keys()
            .filter_map(|file_name| file_name.strip_suffix(".psc").map(str::to_string))
            .collect()
    }

    /// Lists every function and property available on an object of type
    /// `type_name`, including those inherited via `Extends`. A member
    /// declared on `type_name` itself (or an ancestor closer to it) shadows
    /// a same-named member further up the chain, so each name appears at
    /// most once. Returns an empty list if `type_name`'s script can't be
    /// found or parsed. Members are returned in no particular order.
    pub fn list_members(&mut self, type_name: &str) -> Vec<Member> {
        let mut seen = HashSet::new();
        let mut members = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break; // guard against a circular `Extends` chain
            }
            self.ensure_loaded(&name);

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };

            for signature in script.functions.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Function(signature.clone()));
                }
            }
            for signature in script.properties.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Property(signature.clone()));
                }
            }

            current = parent_cache_key(script);
            visited.push(name);
        }

        members
    }

    /// Every property type declared directly on `type_name`'s own script,
    /// in its original case, not extended through `Extends`. Returns an
    /// empty list if `type_name`'s script can't be found or parsed. Used by
    /// the "Circular script dependency" lint
    /// (`papyrus_lints::circular_dependency`) to follow a chain of
    /// `Property` declarations across scripts looking for one that leads
    /// back to the script it started from.
    pub fn property_types(&mut self, type_name: &str) -> Vec<String> {
        let name_lower = type_name.to_ascii_lowercase();
        self.ensure_loaded(&name_lower);

        let Some(script) = self.scripts.get(&name_lower).and_then(Option::as_ref) else {
            return Vec::new();
        };
        script
            .properties
            .values()
            .map(|property| property.type_name.name.clone())
            .collect()
    }

    pub(super) fn lookup_function_cached(
        &self,
        type_name: &str,
        function_name: &str,
    ) -> CacheProbe<Option<FunctionSignature>> {
        let function_key = function_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(None),
                CacheProbe::Hit(Some(script)) => script,
            };
            if let Some(signature) = script.functions.get(&function_key) {
                return CacheProbe::Hit(Some(signature.clone()));
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(None)
    }

    pub(super) fn has_event_cached(
        &self,
        type_name: &str,
        event_name: &str,
    ) -> CacheProbe<Option<bool>> {
        if !self.event_roots_match() {
            return CacheProbe::Miss;
        }
        let type_key = type_name.to_ascii_lowercase();
        let Some(entry) = self.event_index.get(&type_key) else {
            return CacheProbe::Miss;
        };
        if !event_chain_fresh(entry) {
            return CacheProbe::Miss;
        }
        let event_key = event_name.to_ascii_lowercase();
        CacheProbe::Hit(Some(entry.names.contains(&event_key)))
    }

    pub(super) fn is_subtype_cached(&self, sub_type: &str, super_type: &str) -> CacheProbe<bool> {
        let super_lower = super_type.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(sub_type.to_ascii_lowercase());

        while let Some(name) = current {
            if name == super_lower {
                return CacheProbe::Hit(true);
            }
            if visited.contains(&name) {
                break;
            }
            current = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(Some(script)) => script
                    .extends
                    .as_ref()
                    .map(|parent| parent.to_ascii_lowercase()),
                CacheProbe::Hit(None) => None,
            };
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn ancestry_fully_known_cached(&self, type_name: &str) -> CacheProbe<bool> {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                return CacheProbe::Hit(false);
            }
            current = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(Some(script)) => match &script.extends {
                    Some(parent) => Some(parent.to_ascii_lowercase()),
                    None => return CacheProbe::Hit(true),
                },
                CacheProbe::Hit(None) => return CacheProbe::Hit(false),
            };
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn has_property_cached(
        &self,
        type_name: &str,
        property_name: &str,
    ) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script
                .properties
                .contains_key(&property_name.to_ascii_lowercase())
        })
    }

    pub(super) fn has_field_cached(&self, type_name: &str, field_name: &str) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.variables.contains(&field_name.to_ascii_lowercase())
        })
    }

    pub(super) fn has_state_cached(&self, type_name: &str, state_name: &str) -> CacheProbe<bool> {
        self.has_member_cached(type_name, |script| {
            script.states.contains_key(&state_name.to_ascii_lowercase())
        })
    }

    pub(super) fn descendant_targets_state_cached(
        &self,
        type_name: &str,
        state_name: &str,
    ) -> CacheProbe<bool> {
        let Some(index) = &self.descendant_goto_targets else {
            return CacheProbe::Miss;
        };
        CacheProbe::Hit(
            index
                .get(&type_name.to_ascii_lowercase())
                .is_some_and(|targets| targets.contains(&state_name.to_ascii_lowercase())),
        )
    }

    fn has_member_cached(
        &self,
        type_name: &str,
        found: impl Fn(&ScriptFunctions) -> bool,
    ) -> CacheProbe<bool> {
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => return CacheProbe::Hit(false),
                CacheProbe::Hit(Some(script)) => script,
            };
            if found(script) {
                return CacheProbe::Hit(true);
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(false)
    }

    pub(super) fn ancestor_states_cached(
        &self,
        type_name: &str,
    ) -> CacheProbe<Vec<(String, bool)>> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => break,
                CacheProbe::Hit(Some(script)) => script,
            };
            result.extend(
                script
                    .states
                    .iter()
                    .map(|(state, &is_auto)| (state.clone(), is_auto)),
            );
            current = script
                .extends
                .as_ref()
                .map(|parent| parent.to_ascii_lowercase());
            visited.push(name);
        }

        CacheProbe::Hit(result)
    }

    pub(super) fn list_members_cached(&self, type_name: &str) -> CacheProbe<Vec<Member>> {
        let mut seen = HashSet::new();
        let mut members = Vec::new();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());

        while let Some(name) = current {
            if visited.contains(&name) {
                break;
            }
            let script = match cached_script(self, &name) {
                CacheProbe::Miss => return CacheProbe::Miss,
                CacheProbe::Hit(None) => break,
                CacheProbe::Hit(Some(script)) => script,
            };
            for signature in script.functions.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Function(signature.clone()));
                }
            }
            for signature in script.properties.values() {
                if seen.insert(signature.name.to_ascii_lowercase()) {
                    members.push(Member::Property(signature.clone()));
                }
            }
            current = parent_cache_key(script);
            visited.push(name);
        }

        CacheProbe::Hit(members)
    }

    pub(super) fn property_types_cached(&self, type_name: &str) -> CacheProbe<Vec<String>> {
        let name_lower = type_name.to_ascii_lowercase();
        match cached_script(self, &name_lower) {
            CacheProbe::Miss => CacheProbe::Miss,
            CacheProbe::Hit(None) => CacheProbe::Hit(Vec::new()),
            CacheProbe::Hit(Some(script)) => CacheProbe::Hit(
                script
                    .properties
                    .values()
                    .map(|property| property.type_name.name.clone())
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
#[path = "ancestry_tests.rs"]
mod tests;

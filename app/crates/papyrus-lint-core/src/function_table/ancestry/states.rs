//! State lookup and descendant `GoToState` target indexing.

use std::collections::{HashMap, HashSet};

use super::{parent_cache_key, FunctionTable};

impl FunctionTable {
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
}

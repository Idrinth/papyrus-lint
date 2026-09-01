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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

use papyrus_lints::argument_types::ParamInfo;
use papyrus_parser::ast::{FunctionDecl, PropertyDecl, Script, TypeName};

use crate::source_encoding::read_psc_source;

use crate::script_locator::find_psc_file;

/// The parameters (name and type) and return type of a single function, as
/// declared on a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<TypeName>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
    /// The name of the `State` block this signature was resolved from, or
    /// `None` when it comes from the script's empty state — either because
    /// it's declared directly on the script, or because no state overrides
    /// it (see [`ScriptFunctions::from_script`], which prefers the empty
    /// state's declaration whenever both exist, since that's the signature
    /// every ordinary call site resolves against per the language's state
    /// machine).
    pub state: Option<String>,
}

impl FunctionSignature {
    fn from_decl(decl: &FunctionDecl) -> Self {
        FunctionSignature {
            name: decl.name.clone(),
            params: decl
                .params
                .iter()
                .map(|p| ParamInfo {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                })
                .collect(),
            return_type: decl.return_type.clone(),
            is_global: decl.is_global,
            is_native: decl.is_native,
            is_event: decl.is_event,
            state: decl.state.clone(),
        }
    }
}

/// The declared type of a single property, as declared on a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PropertySignature {
    pub name: String,
    pub type_name: TypeName,
}

impl PropertySignature {
    fn from_decl(decl: &PropertyDecl) -> Self {
        PropertySignature {
            name: decl.name.clone(),
            type_name: decl.type_name.clone(),
        }
    }
}

/// A single function or property available on a script type, as returned
/// by [`FunctionTable::list_members`] to drive editor autocompletion.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Member {
    Function(FunctionSignature),
    Property(PropertySignature),
}

impl Member {
    /// The member's declared name, in its original case.
    pub fn name(&self) -> &str {
        match self {
            Member::Function(signature) => &signature.name,
            Member::Property(signature) => &signature.name,
        }
    }
}

/// The functions and properties declared directly on one script, plus the
/// name of the script it extends (if any), so a lookup can walk the
/// inheritance chain.
struct ScriptFunctions {
    extends: Option<String>,
    functions: HashMap<String, FunctionSignature>,
    properties: HashMap<String, PropertySignature>,
    /// Each named `State` declared directly on this script, lowercased,
    /// mapped to whether it's marked `Auto`. Used by [`FunctionTable::has_state`]
    /// and [`FunctionTable::ancestor_states`].
    states: HashMap<String, bool>,
}

impl ScriptFunctions {
    fn from_script(script: &Script) -> Self {
        let mut states: HashMap<String, bool> = HashMap::new();
        for state in &script.states {
            let is_auto = states
                .entry(state.name.to_ascii_lowercase())
                .or_insert(false);
            *is_auto |= state.is_auto;
        }
        let mut functions: HashMap<String, FunctionSignature> = script
            .functions
            .iter()
            .map(|f| (f.name.to_ascii_lowercase(), FunctionSignature::from_decl(f)))
            .collect();
        // A function declared only inside a `State` block (with no
        // matching declaration in the empty state) is still a real,
        // callable member of the script, so it belongs in the function
        // list too — callers just haven't declared its canonical empty
        // state version. A same-named empty state declaration always wins
        // over a state override here, since that's the signature every
        // ordinary (not-in-that-state) call site actually resolves
        // against.
        for state in &script.states {
            for f in &state.functions {
                functions
                    .entry(f.name.to_ascii_lowercase())
                    .or_insert_with(|| FunctionSignature::from_decl(f));
            }
        }
        let properties = script
            .properties
            .iter()
            .map(|p| (p.name.to_ascii_lowercase(), PropertySignature::from_decl(p)))
            .collect();

        ScriptFunctions {
            extends: script.extends.clone(),
            functions,
            properties,
            states,
        }
    }
}

/// Lazily-populated, cross-file lookup table of function signatures, keyed
/// by object (script) type name.
///
/// Each type name is resolved to a `.psc` file at most once: the file is
/// located with [`find_psc_file`], parsed, and its function signatures
/// (along with its `Extends` parent) are cached. A type that can't be
/// found or fails to parse is cached as unresolved so repeated lookups
/// don't retry the filesystem or parser.
pub struct FunctionTable {
    root: PathBuf,
    additional_roots: Vec<String>,
    scripts: HashMap<String, Option<ScriptFunctions>>,
}

impl FunctionTable {
    /// Creates an empty table that resolves script names against
    /// `scripts/source` / `source/scripts` under `root`.
    pub fn new(root: PathBuf) -> Self {
        FunctionTable {
            root,
            additional_roots: Vec::new(),
            scripts: HashMap::new(),
        }
    }

    /// Creates an empty table that also searches `additional_roots` (see
    /// [`crate::config::load_script_roots`]/[`crate::script_locator::find_psc_file`])
    /// alongside `scripts/source` / `source/scripts` under `root`.
    pub fn new_with_additional_roots(root: PathBuf, additional_roots: Vec<String>) -> Self {
        FunctionTable {
            root,
            additional_roots,
            scripts: HashMap::new(),
        }
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

            current = script.extends.clone();
            visited.push(name);
        }

        None
    }

    /// Whether `sub_type`'s script is, or extends (directly or
    /// transitively), `super_type`. Both names are matched
    /// case-insensitively. When a type along the way isn't a script in the
    /// project (e.g. a native engine type like `Actor` or `ObjectReference`,
    /// whose own `Extends` chain isn't declared anywhere in the project),
    /// falls back to [`crate::native_types::parent_of`] for it rather than
    /// giving up; returns `false` only once neither the project nor that
    /// fallback can say what a type in the chain extends before reaching
    /// `super_type`.
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

            current = match self.scripts.get(&name).and_then(Option::as_ref) {
                Some(script) => script.extends.as_ref().map(|e| e.to_ascii_lowercase()),
                None => crate::native_types::parent_of(&name).map(str::to_string),
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

            current = script.extends.clone();
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

            current = script.extends.clone();
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

            // Lowercased, unlike the other walks in this file: those only
            // ever check for a match or stop at the first one found, so a
            // casing mismatch against `visited` costs at most a redundant
            // extra step. This walk instead accumulates every step's
            // states, where the same mismatch would double-count an
            // ancestor's states whenever its `Extends` target's declared
            // casing doesn't match `visited`'s.
            current = script.extends.as_ref().map(|e| e.to_ascii_lowercase());
            visited.push(name);
        }

        result
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

            current = script.extends.clone();
            visited.push(name);
        }

        members
    }

    /// Whether a script named `type_name` can be located at all: either
    /// found under the project root (regardless of whether it parses
    /// cleanly), or known as a native singleton script always called
    /// through its literal name (e.g. `Game`, `Utility`, `Debug`; see
    /// [`crate::native_globals`]). Matched case-insensitively. Used by the
    /// "Unresolved script reference" lint
    /// (`papyrus_lints::unresolved_script`) to flag a call like
    /// `MyMissingScript.DoThing()`.
    pub fn script_exists(&mut self, type_name: &str) -> bool {
        let name_lower = type_name.to_ascii_lowercase();
        find_psc_file(&self.root, &name_lower, &self.additional_roots).is_some()
            || crate::native_globals::is_known(&name_lower)
    }

    /// Parses and caches the script named `name_lower`, if it hasn't been
    /// already. Reuses the on-disk [`crate::ast_cache`] when the script's
    /// content and modification time haven't changed since it was last
    /// parsed, so repeatedly resolving the same cross-script lookup (across
    /// separate CLI invocations, or separate desktop app commands) skips
    /// re-parsing it.
    fn ensure_loaded(&mut self, name_lower: &str) {
        if self.scripts.contains_key(name_lower) {
            return;
        }

        let script = find_psc_file(&self.root, name_lower, &self.additional_roots)
            .and_then(|path| {
                let source = read_psc_source(&path).ok()?;
                if let Some(cached) = crate::ast_cache::get(&path, &source) {
                    return Some(cached);
                }
                let parsed = papyrus_parser::parse(&source).ok()?;
                crate::ast_cache::put(&path, &source, &parsed);
                Some(parsed)
            })
            .map(|script| ScriptFunctions::from_script(&script));

        self.scripts.insert(name_lower.to_string(), script);
    }
}

/// Lets the "Argument type check" lint (`papyrus_lints::argument_types`)
/// resolve calls to functions declared on other scripts through this
/// table.
impl papyrus_lints::argument_types::ExternalSignatures for FunctionTable {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.params)
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        self.is_subtype(sub_type, super_type)
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        self.has_property(type_name, property_name)
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        self.script_exists(type_name)
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        let name_lower = type_name.to_ascii_lowercase();
        matches!(
            name_lower.as_str(),
            "int" | "float" | "bool" | "string" | "var"
        ) || crate::native_types::is_known(&name_lower)
            || self.script_exists(type_name)
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        self.has_state(type_name, state_name)
    }

    fn ancestor_states(&mut self, type_name: &str) -> Vec<(String, bool)> {
        self.ancestor_states(type_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn write_script(dir: &Path, name: &str, contents: &str) {
        let source_dir = dir.join("scripts/source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        fs::write(source_dir.join(format!("{name}.psc")), contents)
            .expect("failed to write test script file");
    }

    #[test]
    fn finds_function_declared_directly_on_the_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nInt Function Bar(Float a, String b)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let signature = table
            .lookup_function("Foo", "Bar")
            .expect("function should be found");

        assert_eq!(signature.name, "Bar");
        assert_eq!(
            signature.return_type,
            Some(TypeName {
                name: "Int".to_string(),
                is_array: false,
            })
        );
        assert_eq!(
            signature.params,
            vec![
                ParamInfo {
                    name: "a".to_string(),
                    type_name: TypeName {
                        name: "Float".to_string(),
                        is_array: false,
                    },
                },
                ParamInfo {
                    name: "b".to_string(),
                    type_name: TypeName {
                        name: "String".to_string(),
                        is_array: false,
                    },
                },
            ]
        );
        assert_eq!(signature.state, None);
    }

    #[test]
    fn finds_a_function_declared_only_inside_a_state() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nState Loud\n    Int Function Bar(Float a)\n    EndFunction\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let signature = table
            .lookup_function("Foo", "Bar")
            .expect("state-declared function should be found");

        assert_eq!(signature.name, "Bar");
        assert_eq!(signature.state.as_deref(), Some("Loud"));
    }

    #[test]
    fn prefers_the_empty_state_signature_over_a_same_named_state_override() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nInt Function Bar()\n    Return 1\nEndFunction\n\nState Loud\n    Int Function Bar()\n        Return 2\n    EndFunction\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let signature = table
            .lookup_function("Foo", "Bar")
            .expect("function should be found");

        assert_eq!(signature.state, None);
    }

    #[test]
    fn finds_function_inherited_through_extends_chain() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Grandparent",
            "ScriptName Grandparent\n\nBool Function IsAwesome()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "Middle",
            "ScriptName Middle Extends Grandparent\n",
        );
        write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let signature = table
            .lookup_function("Child", "IsAwesome")
            .expect("inherited function should be found");

        assert_eq!(signature.name, "IsAwesome");
        assert_eq!(
            signature.return_type,
            Some(TypeName {
                name: "Bool".to_string(),
                is_array: false,
            })
        );
    }

    #[test]
    fn type_and_function_names_are_case_insensitive() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.lookup_function("fOO", "bAR").is_some());
    }

    #[test]
    fn returns_none_for_unknown_function() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Foo", "ScriptName Foo\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.lookup_function("Foo", "DoesNotExist").is_none());
    }

    #[test]
    fn new_with_additional_roots_resolves_a_script_outside_the_conventional_dirs() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let shared = tempfile::tempdir().expect("failed to create temp dir");
        fs::write(
            shared.path().join("Shared.psc"),
            "ScriptName Shared\n\nInt Function DoThing()\nEndFunction\n",
        )
        .expect("failed to write shared script");

        let mut table = FunctionTable::new_with_additional_roots(
            root.path().to_path_buf(),
            vec![shared.path().to_string_lossy().into_owned()],
        );

        let signature = table
            .lookup_function("Shared", "DoThing")
            .expect("function should be found via the additional root");
        assert_eq!(signature.name, "DoThing");
    }

    #[test]
    fn returns_none_when_script_file_cannot_be_found() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.lookup_function("Missing", "Anything").is_none());
    }

    #[test]
    fn caches_parsed_scripts_across_lookups() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        assert!(table.lookup_function("Foo", "Bar").is_some());

        // Remove the backing file: a cached lookup must not touch disk again.
        fs::remove_file(root.path().join("scripts/source/Foo.psc"))
            .expect("failed to remove script file");

        assert!(table.lookup_function("Foo", "Bar").is_some());
    }

    #[test]
    fn does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "A", "ScriptName A Extends B\n");
        write_script(root.path(), "B", "ScriptName B Extends A\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.lookup_function("A", "Anything").is_none());
    }

    #[test]
    fn is_subtype_true_for_direct_and_transitive_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Form", "ScriptName Form\n");
        write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");
        write_script(
            root.path(),
            "ClothingArmor",
            "ScriptName ClothingArmor Extends Armor\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.is_subtype("Armor", "Form"));
        assert!(table.is_subtype("ClothingArmor", "Form"));
        assert!(table.is_subtype("armor", "form"));
        assert!(table.is_subtype("Form", "Form"));
    }

    #[test]
    fn is_subtype_false_for_unrelated_or_unresolvable_types() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Form", "ScriptName Form\n");
        write_script(root.path(), "Weapon", "ScriptName Weapon Extends Form\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.is_subtype("Form", "Weapon"));
        assert!(!table.is_subtype("Weapon", "Armor"));
        assert!(!table.is_subtype("Missing", "Form"));
    }

    #[test]
    fn is_subtype_resolves_native_engine_types_with_no_project_script() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        // None of `Actor`, `ObjectReference`, `Form`, `Spell` or `MagicItem`
        // have a `.psc` anywhere under `root` (typical for a mod project,
        // which doesn't ship copies of the game's own scripts), so this can
        // only pass via the native type fallback.
        assert!(table.is_subtype("Actor", "ObjectReference"));
        assert!(table.is_subtype("Actor", "Form"));
        assert!(table.is_subtype("Spell", "Form"));
        assert!(!table.is_subtype("Form", "Actor"));
        assert!(!table.is_subtype("Spell", "ObjectReference"));
    }

    #[test]
    fn is_subtype_falls_back_to_native_types_past_a_project_scripts_extends_chain() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        // `MyQuestScript` is a project script, but the `Quest` it extends is
        // the native engine type and has no `.psc` under `root`.
        write_script(
            root.path(),
            "MyQuestScript",
            "ScriptName MyQuestScript Extends Quest\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.is_subtype("MyQuestScript", "Quest"));
        assert!(table.is_subtype("MyQuestScript", "Form"));
    }

    #[test]
    fn is_subtype_does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "A", "ScriptName A Extends B\n");
        write_script(root.path(), "B", "ScriptName B Extends A\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.is_subtype("A", "SomethingElse"));
    }

    #[test]
    fn has_property_true_for_a_property_declared_directly_on_the_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nInt Property MyValue Auto\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.has_property("Foo", "MyValue"));
        assert!(table.has_property("foo", "myvalue"));
    }

    #[test]
    fn has_property_true_for_a_property_inherited_through_extends_chain() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Grandparent",
            "ScriptName Grandparent\n\nBool Property IsAwesome Auto\n",
        );
        write_script(
            root.path(),
            "Middle",
            "ScriptName Middle Extends Grandparent\n",
        );
        write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.has_property("Child", "IsAwesome"));
    }

    #[test]
    fn has_property_false_for_unrelated_or_unresolvable_types() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nInt Property MyValue Auto\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.has_property("Foo", "DoesNotExist"));
        assert!(!table.has_property("Missing", "Anything"));
    }

    #[test]
    fn has_property_does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "A", "ScriptName A Extends B\n");
        write_script(root.path(), "B", "ScriptName B Extends A\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.has_property("A", "Anything"));
    }

    #[test]
    fn has_state_true_for_a_state_declared_directly_on_the_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nState Active\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.has_state("Foo", "Active"));
        assert!(table.has_state("foo", "active"));
    }

    #[test]
    fn has_state_true_for_a_state_declared_on_an_ancestor() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Grandparent",
            "ScriptName Grandparent\n\nState Active\nEndState\n",
        );
        write_script(
            root.path(),
            "Middle",
            "ScriptName Middle Extends Grandparent\n",
        );
        write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.has_state("Child", "Active"));
    }

    #[test]
    fn has_state_false_for_unrelated_or_unresolvable_types() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nState Active\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.has_state("Foo", "DoesNotExist"));
        assert!(!table.has_state("Missing", "Anything"));
    }

    #[test]
    fn has_state_does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "A", "ScriptName A Extends B\n");
        write_script(root.path(), "B", "ScriptName B Extends A\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.has_state("A", "Anything"));
    }

    #[test]
    fn ancestor_states_includes_the_types_own_and_inherited_states() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Base",
            "ScriptName Base\n\nAuto State Idle\nEndState\n",
        );
        write_script(
            root.path(),
            "Child",
            "ScriptName Child Extends Base\n\nState Active\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let mut states = table.ancestor_states("Child");
        states.sort();

        assert_eq!(
            states,
            vec![("active".to_string(), false), ("idle".to_string(), true)]
        );
    }

    #[test]
    fn ancestor_states_is_empty_for_an_unresolvable_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.ancestor_states("Missing").is_empty());
    }

    #[test]
    fn ancestor_states_does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "A",
            "ScriptName A Extends B\n\nState FromA\nEndState\n",
        );
        write_script(
            root.path(),
            "B",
            "ScriptName B Extends A\n\nState FromB\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let mut states = table.ancestor_states("A");
        states.sort();

        assert_eq!(
            states,
            vec![("froma".to_string(), false), ("fromb".to_string(), false)]
        );
    }

    #[test]
    fn list_members_includes_functions_and_properties_declared_directly_on_the_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nInt Property MyValue Auto\n\nInt Function Bar(Float a)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("Foo");

        assert_eq!(members.len(), 2);
        assert!(members.iter().any(|m| matches!(
            m,
            Member::Function(signature) if signature.name == "Bar"
        )));
        assert!(members.iter().any(|m| matches!(
            m,
            Member::Property(signature) if signature.name == "MyValue"
        )));
    }

    #[test]
    fn list_members_includes_a_function_declared_only_inside_a_state() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Foo",
            "ScriptName Foo\n\nState Loud\n    Function Bar()\n    EndFunction\nEndState\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("Foo");

        assert!(members.iter().any(|m| matches!(
            m,
            Member::Function(signature) if signature.name == "Bar" && signature.state.as_deref() == Some("Loud")
        )));
    }

    #[test]
    fn list_members_includes_members_inherited_through_extends_chain() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Grandparent",
            "ScriptName Grandparent\n\nBool Property IsAwesome Auto\n\nFunction DoThing()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "Middle",
            "ScriptName Middle Extends Grandparent\n\nFunction DoOtherThing()\nEndFunction\n",
        );
        write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("Child");

        let names: HashSet<_> = members.iter().map(Member::name).collect();
        assert_eq!(
            names,
            HashSet::from(["IsAwesome", "DoThing", "DoOtherThing"])
        );
    }

    #[test]
    fn list_members_stops_at_a_circular_extends_chain() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "A",
            "ScriptName A Extends B\n\nFunction FromA()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "B",
            "ScriptName B Extends A\n\nFunction FromB()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("A");

        let names: HashSet<_> = members.iter().map(Member::name).collect();
        assert_eq!(names, HashSet::from(["FromA", "FromB"]));
    }

    #[test]
    fn list_members_lets_a_closer_declaration_shadow_an_ancestors_member_of_the_same_name() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Base",
            "ScriptName Base\n\nFunction DoThing()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "Child",
            "ScriptName Child Extends Base\n\nBool Function DoThing()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("Child");

        let matches: Vec<_> = members
            .iter()
            .filter(|m| m.name().eq_ignore_ascii_case("DoThing"))
            .collect();
        assert_eq!(matches.len(), 1);
        assert!(matches!(
            matches[0],
            Member::Function(signature) if signature.return_type == Some(TypeName {
                name: "Bool".to_string(),
                is_array: false,
            })
        ));
    }

    #[test]
    fn list_members_is_empty_for_an_unresolvable_type() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.list_members("Missing").is_empty());
    }

    #[test]
    fn list_members_does_not_infinite_loop_on_circular_extends() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "A",
            "ScriptName A Extends B\n\nFunction DoA()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "B",
            "ScriptName B Extends A\n\nFunction DoB()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let members = table.list_members("A");
        let names: HashSet<_> = members.iter().map(Member::name).collect();

        assert_eq!(names, HashSet::from(["DoA", "DoB"]));
    }

    #[test]
    fn flags_a_local_variable_shadowing_a_parent_property_through_the_shadowing_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "BaseScript",
            "ScriptName BaseScript\n\nInt Property MyValue Auto\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::local_variable_shadowing::check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("inherited from a parent script"));
    }

    #[test]
    fn accepts_an_actor_argument_for_an_object_reference_parameter_with_no_native_scripts_in_project(
    ) {
        // Regression test: `Actor`/`ObjectReference`/`Form`/`Spell` are
        // native engine types with no `.psc` under `root` (the project
        // ships none of the game's own scripts), so this can only pass via
        // `FunctionTable::is_subtype`'s native type fallback.
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "UpcastProbe",
            "ScriptName UpcastProbe extends Quest\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::argument_types::check_with(
            r#"
ScriptName UpcastProbe extends Quest

ObjectReference Property AnObjRef Auto
Actor           Property AnActor  Auto
Form            Property AForm    Auto
Spell           Property ASpell   Auto

Function Takes(ObjectReference akRef)
EndFunction

Function Probe()
    Takes(AnObjRef)
    Takes(AnActor)
    Takes(AForm)
    Takes(ASpell)
EndFunction
"#,
            &mut table,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("Takes"));
        assert!(diagnostics[0].message.contains("got Form"));
        assert!(diagnostics[1].message.contains("Takes"));
        assert!(diagnostics[1].message.contains("got Spell"));
    }

    #[test]
    fn flags_a_cast_to_a_native_ancestor_type_through_the_useless_downcast_lint() {
        // Regression test: `Actor`/`ObjectReference` are native engine types
        // with no `.psc` under `root`, so this can only pass via
        // `FunctionTable::is_subtype`'s native type fallback.
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::useless_downcast::check_with(
            "ScriptName Example\n\nFunction Test(Actor dude)\n    Foo(dude as ObjectReference)\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("'Actor' already extends 'ObjectReference'"));
    }

    #[test]
    fn resolves_an_armor_argument_for_a_form_parameter_through_the_argument_type_check_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Form", "ScriptName Form\n");
        write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");
        write_script(
            root.path(),
            "ObjectReference",
            "ScriptName ObjectReference\n\nInt Function GetItemCount(Form akItem)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::argument_types::check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyArmor)\nEndFunction\n",
            &mut table,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn resolves_an_armor_return_value_for_a_form_return_type_through_the_return_type_check_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Form", "ScriptName Form\n");
        write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::return_types::check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nForm Function Test()\n    Return MyArmor\nEndFunction\n",
            &mut table,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_an_unrelated_return_type_through_the_return_type_check_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Form", "ScriptName Form\n");
        write_script(root.path(), "Weapon", "ScriptName Weapon Extends Form\n");
        write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::return_types::check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nArmor Function Test()\n    Return MyWeapon\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("declares return type Armor"));
        assert!(diagnostics[0].message.contains("returns Weapon"));
    }

    #[test]
    fn drives_the_function_override_lint_across_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "ParentScript",
            "ScriptName ParentScript\n\nFunction DoThing()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::function_override::check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'DoThing'"));
        assert!(diagnostics[0].message.contains("'ParentScript'"));
    }

    #[test]
    fn finds_an_override_inherited_transitively_through_the_function_override_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Grandparent",
            "ScriptName Grandparent\n\nFunction DoThing()\nEndFunction\n",
        );
        write_script(
            root.path(),
            "Middle",
            "ScriptName Middle Extends Grandparent\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::function_override::check_with(
            "ScriptName Example Extends Middle\n\nFunction DoThing()\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("'DoThing'"));
    }

    #[test]
    fn drives_the_argument_naming_lint_across_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "ParentScript",
            "ScriptName ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::argument_naming::check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Parameter 1 of 'DoThing'"));
        assert!(diagnostics[0].message.contains("named 'akRef'"));
        assert!(diagnostics[0].message.contains("names it 'akTarget'"));
    }

    #[test]
    fn drives_the_argument_type_check_lint_across_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Greeter",
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::argument_types::check_with(
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
        assert!(diagnostics[0].message.contains("expects String"));
        assert!(diagnostics[0].message.contains("got Int"));
    }

    #[test]
    fn script_exists_true_for_a_script_found_under_the_project_root() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(root.path(), "Foo", "ScriptName Foo\n");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.script_exists("Foo"));
        assert!(table.script_exists("foo"));
    }

    #[test]
    fn script_exists_true_for_a_known_native_singleton_script() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(table.script_exists("Game"));
        assert!(table.script_exists("utility"));
        assert!(table.script_exists("Debug"));
    }

    #[test]
    fn script_exists_false_for_a_script_that_cannot_be_found() {
        let root = tempfile::tempdir().expect("failed to create temp dir");

        let mut table = FunctionTable::new(root.path().to_path_buf());

        assert!(!table.script_exists("MyMissingScript"));
    }

    #[test]
    fn drives_the_unresolved_script_lint_across_scripts() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Greeter",
            "ScriptName Greeter\n\nFunction Greet()\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::unresolved_script::check_with(
            "ScriptName Example\n\nFunction Test()\n    Greeter.Greet()\n    Utility.Wait(1.0)\n    MyMissingScript.DoThing()\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Script 'MyMissingScript' could not be located"));
    }

    #[test]
    fn drives_a_named_argument_check_across_scripts_through_the_argument_type_check_lint() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        write_script(
            root.path(),
            "Greeter",
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );

        let mut table = FunctionTable::new(root.path().to_path_buf());
        let diagnostics = papyrus_lints::argument_types::check_with(
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(name = 1)\nEndFunction\n",
            &mut table,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
        assert!(diagnostics[0].message.contains("expects String"));
        assert!(diagnostics[0].message.contains("got Int"));
    }
}

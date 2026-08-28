//! Builds a lookup table of function signatures (parameter and return
//! types) for Papyrus object types, by locating and parsing their source
//! scripts on demand.
//!
//! Resolving a call like `SomeObject.SomeFunction(...)` requires knowing
//! the argument and return types declared on `SomeObject`'s script — and,
//! since Papyrus scripts inherit via `Extends`, potentially on any of its
//! ancestors too. [`FunctionTable`] finds and parses those scripts (using
//! [`crate::script_locator`]) at most once per type name and caches the
//! result, so looking up functions while linting many other files stays
//! fast.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use papyrus_parser::ast::{FunctionDecl, Script, TypeName};

use crate::script_locator::find_psc_file;

/// The parameter and return types of a single function, as declared on a
/// script.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub name: String,
    pub param_types: Vec<TypeName>,
    pub return_type: Option<TypeName>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
}

impl FunctionSignature {
    fn from_decl(decl: &FunctionDecl) -> Self {
        FunctionSignature {
            name: decl.name.clone(),
            param_types: decl.params.iter().map(|p| p.type_name.clone()).collect(),
            return_type: decl.return_type.clone(),
            is_global: decl.is_global,
            is_native: decl.is_native,
            is_event: decl.is_event,
        }
    }
}

/// The functions and properties declared directly on one script, plus the
/// name of the script it extends (if any), so a lookup can walk the
/// inheritance chain.
struct ScriptFunctions {
    extends: Option<String>,
    functions: HashMap<String, FunctionSignature>,
    properties: HashSet<String>,
}

impl ScriptFunctions {
    fn from_script(script: &Script) -> Self {
        let functions = script
            .functions
            .iter()
            .map(|f| (f.name.to_ascii_lowercase(), FunctionSignature::from_decl(f)))
            .collect();
        let properties = script
            .properties
            .iter()
            .map(|p| p.name.to_ascii_lowercase())
            .collect();

        ScriptFunctions {
            extends: script.extends.clone(),
            functions,
            properties,
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
    scripts: HashMap<String, Option<ScriptFunctions>>,
}

impl FunctionTable {
    /// Creates an empty table that resolves script names against
    /// `scripts/source` / `source/scripts` under `root`.
    pub fn new(root: PathBuf) -> Self {
        FunctionTable {
            root,
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
    /// case-insensitively. Returns `false` if `sub_type`'s script (or any
    /// ancestor along the way) can't be found or parsed before reaching
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

            let Some(script) = self.scripts.get(&name).and_then(Option::as_ref) else {
                break;
            };
            current = script.extends.as_ref().map(|e| e.to_ascii_lowercase());
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
            if script.properties.contains(&property_key) {
                return true;
            }

            current = script.extends.clone();
            visited.push(name);
        }

        false
    }

    /// Parses and caches the script named `name_lower`, if it hasn't been
    /// already.
    fn ensure_loaded(&mut self, name_lower: &str) {
        if self.scripts.contains_key(name_lower) {
            return;
        }

        let script = find_psc_file(&self.root, name_lower)
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|source| papyrus_parser::parse(&source).ok())
            .map(|script| ScriptFunctions::from_script(&script));

        self.scripts.insert(name_lower.to_string(), script);
    }
}

/// Lets the "Argument type check" lint (`papyrus_lints::argument_types`)
/// resolve calls to functions declared on other scripts through this
/// table.
impl papyrus_lints::argument_types::ExternalSignatures for FunctionTable {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<TypeName>> {
        self.lookup_function(type_name, function_name)
            .map(|signature| signature.param_types)
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        self.is_subtype(sub_type, super_type)
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        self.has_property(type_name, property_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            signature.param_types,
            vec![
                TypeName {
                    name: "Float".to_string(),
                    is_array: false,
                },
                TypeName {
                    name: "String".to_string(),
                    is_array: false,
                },
            ]
        );
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
}

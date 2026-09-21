use super::super::test_support::{diagnostics_for, write_script};
use super::*;

#[test]
fn function_table_forwards_every_external_signature_lookup() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nInt Property Count Auto ; @private\nString Label = \"\"\n\nAuto State Idle\nEndState\n\nFunction Run(String message) Global ; @deprecated @protected\nEndFunction\n\nInt Function RegisterFoo() ; @nodiscard\n    Count += 1\n    Return Count\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nForm Property Target Auto\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let external: &mut dyn papyrus_lints::ExternalSignatures = &mut table;

    let params = external
        .lookup("child", "run")
        .expect("inherited function should resolve through the adapter");
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name, "message");
    assert_eq!(params[0].type_name.name, "String");
    assert!(external.is_subtype("Child", "Base"));
    assert!(external.has_property("Child", "Count"));
    assert_eq!(
        external.property_access("Child", "Count"),
        Some(papyrus_lints::MemberAccess {
            declaring_type: "base".to_string(),
            access_level: papyrus_parser::ast::AccessLevel::Private,
        })
    );
    assert!(external.has_field("Child", "Label"));
    assert!(external.script_exists("Child"));
    assert!(external.can_resolve_script("Child"));
    assert!(!external.can_resolve_script("DefinitelyMissing"));

    for primitive in ["INT", "float", "Bool", "STRING", "Var"] {
        assert!(external.type_exists(primitive));
    }
    assert!(external.type_exists("Child"));
    assert!(!external.type_exists("DefinitelyMissing"));

    assert!(external.has_state("Child", "Idle"));
    let mut states = external.ancestor_states("Child");
    states.sort();
    assert_eq!(
        states,
        vec![("active".to_string(), false), ("idle".to_string(), true)]
    );
    assert_eq!(external.is_global_function("Child", "Run"), Some(true));
    assert_eq!(
        external.function_access("Child", "Run"),
        Some(papyrus_lints::MemberAccess {
            declaring_type: "base".to_string(),
            access_level: papyrus_parser::ast::AccessLevel::Protected,
        })
    );
    assert_eq!(external.is_deprecated_function("Child", "Run"), Some(true));
    assert_eq!(
        external.is_nodiscard_function("Child", "RegisterFoo"),
        Some(true)
    );
    assert_eq!(
        external.function_has_side_effects("Child", "RegisterFoo"),
        Some(true)
    );
    assert_eq!(external.is_global_function("Child", "Missing"), None);
    assert_eq!(external.is_nodiscard_function("Child", "Missing"), None);
    assert_eq!(external.is_deprecated_function("Child", "Missing"), None);
    assert_eq!(external.function_has_side_effects("Child", "Missing"), None);
    assert!(external.ancestry_fully_known("Child"));
    assert!(!external.ancestry_fully_known("DefinitelyMissing"));
    assert_eq!(external.property_types("Child"), vec!["Form"]);
}

#[test]
fn exposes_the_canonical_side_effect_flag_to_lints() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source = "ScriptName Example\n\nInt Property Count Auto\n\nInt Function Bump()\n    Count += 1\n    Return Count\nEndFunction\n\nFunction Test()\n    Debug.Trace(Bump())\nEndFunction\n";
    write_script(root.path(), "Example", source);

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert_eq!(
        papyrus_lints::ExternalSignatures::function_has_side_effects(&mut table, "Example", "Bump"),
        Some(true)
    );

    let diagnostics = diagnostics_for("debug-side-effects", source, &mut table);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Bump"));
}

#[test]
fn deprecated_function_lint_reads_project_directives() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "LegacyApi",
        "ScriptName LegacyApi\n\nFunction OldWay() ; @deprecated\nEndFunction\n",
    );
    let source = "ScriptName Example\n\nLegacyApi Property Api Auto\n\nFunction Test()\n    Api.OldWay()\nEndFunction\n";
    write_script(root.path(), "Example", source);

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for("deprecated-functions", source, &mut table);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OldWay"));
}

#[test]
fn deprecated_function_lint_reads_bundled_ast_metadata() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source = "ScriptName Example\n\nActor Property Subject Auto\n\nFunction Test()\n    Subject.ModFavorPoints()\nEndFunction\n";
    let mut table = FunctionTable::new(root.path().to_path_buf());

    let diagnostics = diagnostics_for("deprecated-functions", source, &mut table);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("ModFavorPoints"));
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
    let diagnostics = diagnostics_for(
        "local-variable-shadowing",
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
    let diagnostics = papyrus_lints::check_argument_types(
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
    let diagnostics = diagnostics_for(
        "return-types",
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
    let diagnostics = diagnostics_for(
        "return-types",
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
    let diagnostics = diagnostics_for(
        "function-override",
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
    let diagnostics = diagnostics_for(
        "function-override",
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
    let diagnostics = diagnostics_for(
        "argument-naming",
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
    let diagnostics = papyrus_lints::check_argument_types(
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}

#[test]
fn external_signature_trait_reports_builtin_and_resolvable_project_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run() Global\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "FLOAT"
    ));
    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "Actor"
    ));
    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "helpers"
    ));
    assert!(!papyrus_lints::ExternalSignatures::type_exists(
        &mut table,
        "DefinitelyMissing"
    ));
    assert_eq!(
        papyrus_lints::ExternalSignatures::is_global_function(&mut table, "Helpers", "Run"),
        Some(true)
    );
    assert_eq!(
        papyrus_lints::ExternalSignatures::is_global_function(&mut table, "Helpers", "Missing"),
        None
    );
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
    let diagnostics = diagnostics_for(
        "unresolved-script",
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
    let diagnostics = papyrus_lints::check_argument_types(
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(name = 1)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}

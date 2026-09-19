//! Tests that each `config.rules` flag gates only its own lint through
//! [`crate::lint_with_external_arguments`]. These rules need an
//! [`crate::ExternalSignatures`] resolver to ever fire, so they are not
//! covered by [`super::rule_flags`]'s bare-`lint()` loop.

use super::super::*;
use super::support::*;

/// See the note on `each_rule_flag_gates_only_its_own_lint` in
/// [`super::rule_flags`]:
/// `function_override` needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.function_override`
/// gate is checked here instead of in that loop.
#[test]
fn function_override_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithParentFunction,
    );
    assert!(enabled.iter().any(|d| d.rule == function_override::RULE));

    let disabled_config = config_with(|c| c.rules.function_override = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithParentFunction,
    );
    assert!(disabled.iter().all(|d| d.rule != function_override::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `unresolved_script` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.unresolved_script` gate is checked here instead of in the
/// main loop.
#[test]
fn unresolved_script_flag_gates_only_its_own_lint() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    MissingScript.DoThing()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithMissingScript,
    );
    assert!(enabled.iter().any(|d| d.rule == unresolved_script::RULE));

    let disabled_config = config_with(|c| c.rules.unresolved_script = false);
    let disabled =
        lint_with_external_arguments(source, &disabled_config, &mut FakeExternalWithMissingScript);
    assert!(disabled.iter().all(|d| d.rule != unresolved_script::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `circular_dependency` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.circular_dependency` gate is checked here instead of in the
/// main loop. It also defaults to `false` (see `config::Rules`), unlike
/// every rule the main loop covers, so both configs here are built from
/// `config_with` rather than one being `Config::default()`.
#[test]
fn circular_dependency_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nB Property Little Auto\n";

    let enabled_config = config_with(|c| c.rules.circular_dependency = true);
    let enabled = lint_with_external_arguments(
        source,
        &enabled_config,
        &mut FakeExternalWithCircularProperty,
    );
    assert!(enabled.iter().any(|d| d.rule == circular_dependency::RULE));

    let disabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithCircularProperty,
    );
    assert!(disabled.iter().all(|d| d.rule != circular_dependency::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `non_global_function_call` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own
/// `rules.non_global_function_call` gate is checked here instead of in
/// the main loop.
#[test]
fn non_global_function_call_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nFunction Test()\n    MyScript.NotGlobal()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithNonGlobalFunction,
    );
    assert!(enabled
        .iter()
        .any(|d| d.rule == non_global_function_call::RULE));

    let disabled_config = config_with(|c| c.rules.non_global_function_call = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithNonGlobalFunction,
    );
    assert!(disabled
        .iter()
        .all(|d| d.rule != non_global_function_call::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `static_function_call_via_instance` also needs
/// `lint_with_external_arguments`'s `external` resolver to ever fire, so
/// its own `rules.static_function_call_via_instance` gate is checked
/// here instead of in the main loop.
#[test]
fn static_function_call_via_instance_flag_gates_only_its_own_lint() {
    let source =
        "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.IsGlobal()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithGlobalFunction,
    );
    assert!(enabled
        .iter()
        .any(|d| d.rule == static_function_call_via_instance::RULE));

    let disabled_config = config_with(|c| c.rules.static_function_call_via_instance = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithGlobalFunction,
    );
    assert!(disabled
        .iter()
        .all(|d| d.rule != static_function_call_via_instance::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `argument_naming` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.argument_naming`
/// gate is checked here instead of in the main loop.
#[test]
fn argument_naming_flag_gates_only_its_own_lint() {
    let source =
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithRenamedParentParam,
    );
    assert!(enabled.iter().any(|d| d.rule == argument_naming::RULE));

    let disabled_config = config_with(|c| c.rules.argument_naming = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithRenamedParentParam,
    );
    assert!(disabled.iter().all(|d| d.rule != argument_naming::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `impossible_cast` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire (see `impossible_cast`'s module
/// docs), so its own `rules.impossible_cast` gate is checked here
/// instead of in the main loop.
#[test]
fn impossible_cast_flag_gates_only_its_own_lint() {
    let source =
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnrelatedAncestry,
    );
    assert!(enabled.iter().any(|d| d.rule == impossible_cast::RULE));

    let disabled_config = config_with(|c| c.rules.impossible_cast = false);
    let disabled = lint_with_external_arguments(
        source,
        &disabled_config,
        &mut FakeExternalWithUnrelatedAncestry,
    );
    assert!(disabled.iter().all(|d| d.rule != impossible_cast::RULE));
}

/// Like `function_override_flag_gates_only_its_own_lint` above:
/// `unused_import` also needs `lint_with_external_arguments`'s
/// `external` resolver to ever fire, so its own `rules.unused_import`
/// gate is checked here instead of in the main loop.
#[test]
fn unused_import_flag_gates_only_its_own_lint() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let enabled = lint_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
    );
    assert!(enabled.iter().any(|d| d.rule == unused_import::RULE));

    let disabled_config = config_with(|c| c.rules.unused_import = false);
    let disabled =
        lint_with_external_arguments(source, &disabled_config, &mut FakeExternalWithUnusedImport);
    assert!(disabled.iter().all(|d| d.rule != unused_import::RULE));
}

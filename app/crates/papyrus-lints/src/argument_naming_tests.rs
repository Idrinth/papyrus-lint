use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn check_with<E: ExternalSignatures + ?Sized>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}
use crate::external_signatures::ParamInfo;
use papyrus_parser::ast::TypeName;

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(vec![
                ParamInfo {
                    name: "akTarget".to_string(),
                    type_name: TypeName {
                        name: "ObjectReference".to_string(),
                        is_array: false,
                    },
                },
                ParamInfo {
                    name: "aiCount".to_string(),
                    type_name: TypeName {
                        name: "Int".to_string(),
                        is_array: false,
                    },
                },
            ])
        } else {
            None
        }
    }
}

#[test]
fn without_external_never_flags_anything() {
    let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_renamed_parameter_on_an_override() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Parameter 1 of 'DoThing'"));
    assert!(diagnostics[0].message.contains("named 'akRef'"));
    assert!(diagnostics[0].message.contains("names it 'akTarget'"));
}

#[test]
fn does_not_flag_parameter_names_that_only_differ_in_case() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference AKTARGET, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_each_mismatched_parameter_separately() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Parameter 1"));
    assert!(diagnostics[1].message.contains("Parameter 2"));
}

#[test]
fn does_not_flag_a_function_with_no_matching_inherited_name() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction SomethingElse(Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_anything_on_a_script_without_extends() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_declared_only_inside_a_state() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing(ObjectReference akRef, Int aiCount)\n    EndFunction\nEndState\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_compare_parameters_beyond_the_shorter_declarations_count() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn repair_renames_the_overridden_parameter_and_its_uses() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    akRef.Disable()\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);
    assert!(repaired.contains("ObjectReference akTarget"));
    assert!(!repaired.contains("akRef"));
    assert!(repaired.contains("akTarget.Disable()"));
}

#[test]
fn repair_skips_a_rename_that_would_duplicate_a_local() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    ObjectReference akTarget = akRef.GetLinkedRef()\nEndFunction\n";

    assert_eq!(super::repair_with(source, &mut FakeExternal), source);
    let diagnostics = check_with(source, &mut FakeExternal);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("named 'akRef'"));
    assert!(diagnostics[0].message.contains("names it 'akTarget'"));
}

#[test]
fn repair_skips_a_rename_that_would_duplicate_a_nested_local() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    If (aiCount > 0)\n        ObjectReference AkTarget = None\n    Else\n        Int unused = 0\n    EndIf\nEndFunction\n";

    assert_eq!(super::repair_with(source, &mut FakeExternal), source);
    assert_eq!(check_with(source, &mut FakeExternal).len(), 1);
}

#[test]
fn repair_keeps_a_safe_rename_when_another_would_collide() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\n    ObjectReference akTarget = None\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    ObjectReference akTarget = None\nEndFunction\n"
    );
    let remaining = check_with(&repaired, &mut FakeExternal);
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].message.contains("Parameter 1"));
}

#[test]
fn repair_renames_swapped_parameters() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference aiCount, Int akTarget)\n    aiCount.Disable()\n    akTarget = akTarget + 1\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget, Int aiCount)\n    akTarget.Disable()\n    aiCount = aiCount + 1\nEndFunction\n"
    );
}

#[test]
fn repair_does_not_apply_a_rename_that_only_looks_vacated() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int akTarget)\n    Int aiCount = 0\nEndFunction\n";

    assert_eq!(super::repair_with(source, &mut FakeExternal), source);
    assert_eq!(check_with(source, &mut FakeExternal).len(), 2);
}

#[test]
fn repair_leaves_member_names_and_named_argument_labels() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef = None, Int aiCount)\n    OtherRef.akRef.Disable()\n    Helper(akRef = akRef)\n    Helper(aiCount, akRef = akRef)\n    akRef = akRef\n    If (akRef == akRef)\n    EndIf\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget = None, Int aiCount)\n    OtherRef.akRef.Disable()\n    Helper(akRef = akTarget)\n    Helper(aiCount, akRef = akTarget)\n    akTarget = akTarget\n    If (akTarget == akTarget)\n    EndIf\nEndFunction\n"
    );
}

struct MemberExternal {
    property: Option<&'static str>,
    field: Option<&'static str>,
}

impl ExternalSignatures for MemberExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        FakeExternal.lookup(type_name, function_name)
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("ParentScript")
            && self
                .property
                .is_some_and(|name| name.eq_ignore_ascii_case(property_name))
    }

    fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("ParentScript")
            && self
                .field
                .is_some_and(|name| name.eq_ignore_ascii_case(field_name))
    }
}

#[test]
fn repair_skips_a_rename_that_would_shadow_an_own_property() {
    let source = "ScriptName Example Extends ParentScript\n\nInt Property akTarget Auto\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    akTarget = 1\nEndFunction\n";

    assert_eq!(super::repair_with(source, &mut FakeExternal), source);
    let diagnostics = check_with(source, &mut FakeExternal);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("named 'akRef'"));
    assert!(diagnostics[0].message.contains("names it 'akTarget'"));
}

#[test]
fn repair_skips_a_rename_that_would_shadow_an_own_variable() {
    let source = "ScriptName Example Extends ParentScript\n\nObjectReference AkTarget\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    AkTarget = akRef\nEndFunction\n";

    assert_eq!(super::repair_with(source, &mut FakeExternal), source);
    assert_eq!(check_with(source, &mut FakeExternal).len(), 1);
}

#[test]
fn repair_keeps_a_safe_rename_when_another_would_shadow_a_property() {
    let source = "ScriptName Example Extends ParentScript\n\nInt Property akTarget Auto\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\n    akTarget = 1\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nInt Property akTarget Auto\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    akTarget = 1\nEndFunction\n"
    );
    let remaining = check_with(&repaired, &mut FakeExternal);
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].message.contains("Parameter 1"));
}

#[test]
fn repair_does_not_treat_a_member_name_as_vacated() {
    let source = "ScriptName Example Extends ParentScript\n\nInt Property akTarget Auto\n\nFunction DoThing(ObjectReference akRef, Int akTarget)\n    akRef.Disable()\n    akTarget = akTarget + 1\nEndFunction\n";
    let repaired = super::repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nInt Property akTarget Auto\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    akRef.Disable()\n    aiCount = aiCount + 1\nEndFunction\n"
    );
    let remaining = check_with(&repaired, &mut FakeExternal);
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].message.contains("Parameter 1"));
    assert!(remaining[0].message.contains("names it 'akTarget'"));
}

#[test]
fn repair_does_not_treat_an_inherited_property_as_vacated() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int akTarget)\n    akRef.Disable()\n    akTarget = akTarget + 1\nEndFunction\n";
    let mut external = MemberExternal {
        property: Some("AkTarget"),
        field: None,
    };
    let repaired = super::repair_with(source, &mut external);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\n    akRef.Disable()\n    aiCount = aiCount + 1\nEndFunction\n"
    );
    assert_eq!(check_with(&repaired, &mut external).len(), 1);
}

#[test]
fn repair_skips_a_rename_that_would_shadow_an_inherited_field() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\n    akRef.Disable()\nEndFunction\n";
    let mut external = MemberExternal {
        property: None,
        field: Some("aiCount"),
    };
    let repaired = super::repair_with(source, &mut external);

    assert_eq!(
        repaired,
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget, Int aiTotal)\n    akTarget.Disable()\nEndFunction\n"
    );
    let remaining = check_with(&repaired, &mut external);
    assert_eq!(remaining.len(), 1);
    assert!(remaining[0].message.contains("Parameter 2"));
    assert!(remaining[0].message.contains("names it 'aiCount'"));
}

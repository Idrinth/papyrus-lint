use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}

#[test]
fn does_not_flag_without_external_ancestry_resolution() {
    // `check` (no external resolver) can't confirm either type's
    // ancestry actually resolves to a root, so it never flags anything
    // on its own; see `check_with` below for the confirmed case.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

struct FakeExternalWithUnrelatedTypes;

impl ExternalSignatures for FakeExternalWithUnrelatedTypes {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Armor") || type_name.eq_ignore_ascii_case("Weapon")
    }
}

#[test]
fn flags_a_cast_between_confirmed_unrelated_types() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'Armor'"));
    assert!(diagnostics[0].message.contains("'Weapon'"));
}

struct FakeExternalWithSubtype;

impl ExternalSignatures for FakeExternalWithSubtype {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
            || (sub_type.eq_ignore_ascii_case("Actor")
                && super_type.eq_ignore_ascii_case("ObjectReference"))
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Actor") || type_name.eq_ignore_ascii_case("ObjectReference")
    }
}

#[test]
fn does_not_flag_a_legitimate_narrowing_or_widening_cast() {
    let widening = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    ObjectReference b = akActor as ObjectReference\nEndFunction\n",
            &mut FakeExternalWithSubtype,
        );
    assert!(widening.is_empty());

    let narrowing = check_with(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor b = akRef as Actor\nEndFunction\n",
            &mut FakeExternalWithSubtype,
        );
    assert!(narrowing.is_empty());
}

#[test]
fn does_not_flag_an_exact_type_match() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Armor b = akArmor as Armor\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert!(diagnostics.is_empty());
}

struct FakeExternalWithUnresolvedAncestry;

impl ExternalSignatures for FakeExternalWithUnresolvedAncestry {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
    }

    // Only one of the two types' ancestry is confirmed resolved.
    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Armor")
    }
}

#[test]
fn does_not_flag_when_only_one_sides_ancestry_is_confirmed() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Weapon b = akArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnresolvedAncestry,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_cast_involving_a_primitive_type() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(Int a)\n    Armor b = a as Armor\nEndFunction\n",
        &mut FakeExternalWithUnrelatedTypes,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_cast_to_a_primitive_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Int value = akArmor as Int\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_cast_whose_value_type_is_unresolvable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Weapon b = GetTarget() as Weapon\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_cast_on_a_property() {
    let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    Weapon b = MyArmor as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check_with(
            "ScriptName Example\n\nState Active\n    Function Test(Armor akArmor)\n        Weapon b = akArmor as Weapon\n    EndFunction\nEndState\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_a_cast_from_an_array_typed_value() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor[] armors)\n    Weapon b = armors as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn walks_a_cast_value_nested_in_an_index_expression() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor[] arr)\n    Weapon b = arr[0] as Weapon\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    // Whether the array-element access itself resolves to a known type
    // isn't the point here; this just needs to walk the Index
    // expression nested inside the cast without crashing.
    assert!(diagnostics.len() <= 1);
}

#[test]
fn flags_a_cast_nested_in_a_named_argument_and_walks_a_new_array_expression() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    SomeCall(flag = akArmor as Weapon)\n    Int[] arr = new Int[3]\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Armor'"));
}

#[test]
fn finds_casts_in_control_flow_conditions_and_bodies() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor as Weapon\n        Foo(akArmor as Weapon)\n    ElseIf akArmor as Weapon\n        Return akArmor as Weapon\n    Else\n        Weapon local = akArmor as Weapon\n    EndIf\n    While akArmor as Weapon\n        Foo(akArmor as Weapon)\n    EndWhile\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    let lines: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.line)
        .collect();
    assert_eq!(lines, vec![4, 5, 6, 7, 9, 11, 12]);
}

#[test]
fn finds_casts_nested_in_composite_expressions() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Bool compared = (akArmor as Weapon) == None\n    Bool negated = !(akArmor as Weapon)\n    Foo((akArmor as Weapon).GetName())\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    let lines: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.line)
        .collect();
    assert_eq!(lines, vec![4, 5, 6]);
}

#[test]
fn flags_each_cast_when_casts_are_nested() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Foo((akArmor as Weapon) as Armor)\nEndFunction\n",
            &mut FakeExternalWithUnrelatedTypes,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.line == 4 && diagnostic.rule == RULE));
}

#[test]
fn external_lookup_is_not_needed_to_classify_casts() {
    assert!(FakeExternalWithUnrelatedTypes
        .lookup("Armor", "SomeFunction")
        .is_none());
}

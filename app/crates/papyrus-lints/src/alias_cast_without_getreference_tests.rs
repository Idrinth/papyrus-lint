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

#[test]
fn flags_casting_reference_alias_to_actor() {
    let diagnostics = check(
        "ScriptName Example\n\nReferenceAlias Property MyAlias Auto\n\nFunction Bad()\n    (MyAlias as Actor).Kill()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("GetActorReference()"));
}

#[test]
fn flags_casting_alias_to_object_reference() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Bad(Alias akAlias)\n    ObjectReference r = akAlias as ObjectReference\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("GetReference()"));
}

#[test]
fn flags_casting_location_alias_to_location() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Bad(LocationAlias akLoc)\n    Location l = akLoc as Location\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("GetLocation()"));
}

#[test]
fn does_not_flag_alias_cast_to_reference_alias() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fine(Alias akAlias)\n    ReferenceAlias r = akAlias as ReferenceAlias\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_alias_cast_to_custom_alias_type() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fine(Alias akAlias)\n    MyQuestAlias r = akAlias as MyQuestAlias\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_reference_alias_cast_to_location_alias() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fine(ReferenceAlias akAlias)\n    LocationAlias l = akAlias as LocationAlias\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_get_actor_reference() {
    let diagnostics = check(
        "ScriptName Example\n\nReferenceAlias Property MyAlias Auto\n\nFunction Good()\n    Actor a = MyAlias.GetActorReference()\n    If a\n        a.Kill()\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unrelated_actor_cast() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fine(ObjectReference akRef)\n    Actor a = akRef as Actor\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn disable_directives_suppress_only_the_selected_diagnostics() {
    let source = "ScriptName Example\n\nFunction Bad(ReferenceAlias first, ReferenceAlias second)\n    Actor a = first as Actor ; @disable alias-cast-without-getreference\n    Actor b = second as Actor\nEndFunction\n";
    let diagnostics = crate::lint(source, &crate::config::Config::default());

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE || diagnostic.line != 4));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == RULE && diagnostic.line == 5));

    let file_disabled = crate::lint(
        "; @disable-file alias-cast-without-getreference\nScriptName Example\n\nFunction Bad(ReferenceAlias akAlias)\n    Actor a = akAlias as Actor\nEndFunction\n",
        &crate::config::Config::default(),
    );
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = "ScriptName Example\n\nFunction Bad(ReferenceAlias akAlias)\n    Actor a = akAlias as Actor\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.alias_cast_without_getreference = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn ignores_primitive_array_and_unknown_cast_values() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Fine(ReferenceAlias akAlias, ReferenceAlias[] aliases)\n    Bool present = akAlias as Bool\n    Actor fromArray = aliases as Actor\n    Actor unknown = missingValue as Actor\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn local_types_do_not_leak_between_functions() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction First()\n    ReferenceAlias selected\n    Actor a = selected as Actor\nEndFunction\n\nFunction Second()\n    Actor selected\n    Actor a = selected as Actor\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

struct FakeExtendsReferenceAlias;

impl ExternalSignatures for FakeExtendsReferenceAlias {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case(super_type)
            || (sub_type.eq_ignore_ascii_case("MySpecialAlias")
                && super_type.eq_ignore_ascii_case("ReferenceAlias"))
    }
}

#[test]
fn flags_subtype_of_reference_alias_cast_to_actor() {
    let ast = papyrus_parser::parse(
        "ScriptName Example\n\nFunction Bad(MySpecialAlias akAlias)\n    Actor a = akAlias as Actor\nEndFunction\n",
    )
    .ok();
    let diagnostics = super::check(
        "",
        ast.as_ref(),
        None,
        &crate::config::Config::default(),
        &mut FakeExtendsReferenceAlias,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn does_not_flag_subtype_of_reference_alias_cast_to_alias() {
    let ast = papyrus_parser::parse(
        "ScriptName Example\n\nFunction Fine(MySpecialAlias akAlias)\n    Alias a = akAlias as Alias\nEndFunction\n",
    )
    .ok();
    let diagnostics = super::check(
        "",
        ast.as_ref(),
        None,
        &crate::config::Config::default(),
        &mut FakeExtendsReferenceAlias,
    );

    assert!(diagnostics.is_empty());
}

//! `self` (and other values of the linted script's own type) passed for a
//! parameter typed as the script or one of its `Extends` ancestors. See
//! [`super::SelfScript`].

use super::*;

use papyrus_parser::parser::GameEdition;

/// Parses `source` as `mode`'s dialect, failing the test if it doesn't
/// parse (so an empty result can't come from an unparsed script), then
/// checks it against `external`.
fn check_parsed<E: ExternalSignatures>(
    source: &str,
    mode: GameEdition,
    external: &mut E,
) -> Vec<Diagnostic> {
    let parsed = papyrus_parser::parse_with_mode(source, mode);
    let ast = parsed.expect("test source should parse");
    super::check_with(Some(&ast), external)
}

fn check_skyrim(source: &str) -> Vec<Diagnostic> {
    check_parsed(
        source,
        GameEdition::Skyrim,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn check_fallout4<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    check_parsed(source, GameEdition::Fallout4, external)
}

fn check_chain(source: &str, complete: bool) -> Vec<Diagnostic> {
    check_parsed(source, GameEdition::Skyrim, &mut ChainExternal { complete })
}

/// A project where `ChainA Extends ChainB` and `ChainB` is a root, as a
/// `FunctionTable` would resolve it, and `ChainUtil` declares
/// `Takes(ChainB arg)`. The script being linted (e.g. `ChainC Extends
/// ChainA`, an unsaved buffer) is not known to it, so `is_subtype` from
/// that script's own name always fails. With `complete` unset, no
/// ancestry can be walked to a root, as if some script were missing.
struct ChainExternal {
    complete: bool,
}

const CHAIN: [&str; 2] = ["ChainA", "ChainB"];

fn chain_position(type_name: &str) -> Option<usize> {
    CHAIN
        .iter()
        .position(|name| name.eq_ignore_ascii_case(type_name))
}

impl ExternalSignatures for ChainExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if !type_name.eq_ignore_ascii_case("ChainUtil")
            || !function_name.eq_ignore_ascii_case("Takes")
        {
            return None;
        }
        Some(vec![ParamInfo {
            name: "arg".to_string(),
            type_name: TypeName {
                name: "ChainB".to_string(),
                is_array: false,
            },
        }])
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        if !self.complete {
            return false;
        }
        match (chain_position(sub_type), chain_position(super_type)) {
            (Some(sub), Some(sup)) => sub <= sup,
            _ => false,
        }
    }

    fn ancestry_fully_known(&mut self, type_name: &str) -> bool {
        self.complete && chain_position(type_name).is_some()
    }
}

#[test]
fn accepts_self_for_a_parameter_typed_as_the_direct_parent_without_a_resolver() {
    let diagnostics = check_skyrim(
        r#"
ScriptName ChildScript Extends ParentScript

Function Takes(ParentScript arg)
EndFunction

Function Test()
    Takes(self)
    self.Takes(self)
    Takes(arg = self)
EndFunction
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_self_for_its_own_type_and_parent_case_insensitively() {
    let diagnostics = check_skyrim(
        r#"
ScriptName childscript Extends PARENTSCRIPT

Function TakesParent(parentscript arg)
EndFunction

Function TakesSelf(CHILDSCRIPT arg)
EndFunction

Function Test()
    TAKESPARENT(SELF)
    takesself(Self)
EndFunction
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_a_variable_typed_as_the_script_itself_for_its_parent() {
    let diagnostics = check_skyrim(
        r#"
ScriptName ChildScript Extends ParentScript

Function Takes(ParentScript arg)
EndFunction

Function Test(ChildScript other)
    Takes(other)
EndFunction
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_self_for_a_grandparent_resolved_through_the_extends_chain() {
    let source = r#"
ScriptName ChainC Extends ChainA

ChainUtil Property Util Auto

Function LocalTakes(ChainB arg)
EndFunction

Function Test()
    Util.Takes(self)
    LocalTakes(self)
    self.LocalTakes(self)
EndFunction
"#;

    let diagnostics = check_chain(source, true);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_self_for_a_function_declared_on_another_script() {
    let source = r#"
ScriptName ChildOfB Extends ChainB

ChainUtil Property Util Auto

Function Test()
    Util.Takes(self)
EndFunction
"#;

    let diagnostics = check_chain(source, true);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn stays_quiet_for_self_when_the_ancestry_cannot_be_resolved() {
    let source = r#"
ScriptName ChainC Extends ChainA

ChainUtil Property Util Auto

Function TakesOther(OtherScript arg)
EndFunction

Function Test()
    Util.Takes(self)
    TakesOther(self)
EndFunction
"#;

    let diagnostics = check_chain(source, false);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn still_flags_self_for_a_type_outside_a_fully_resolved_ancestry() {
    let source = r#"
ScriptName ChainC Extends ChainA

Function TakesOther(OtherScript arg)
EndFunction

Function Test()
    TakesOther(self)
EndFunction
"#;

    let diagnostics = check_chain(source, true);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects OtherScript"));
    assert!(diagnostics[0].message.contains("got ChainC"));
}

#[test]
fn still_flags_a_parent_typed_value_passed_for_the_child_type() {
    let diagnostics = check_skyrim(
        r#"
ScriptName ChildScript Extends ParentScript

Function TakesChild(ChildScript arg)
EndFunction

Function Test(ParentScript other)
    TakesChild(other)
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects ChildScript"));
    assert!(diagnostics[0].message.contains("got ParentScript"));
}

#[test]
fn still_flags_self_for_an_unrelated_type_when_the_script_extends_nothing() {
    let diagnostics = check_skyrim(
        r#"
ScriptName RootScript

Function Takes(OtherScript arg)
EndFunction

Function Test()
    Takes(self)
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects OtherScript"));
    assert!(diagnostics[0].message.contains("got RootScript"));
}

#[test]
fn still_flags_self_for_a_primitive_parameter() {
    let diagnostics = check_skyrim(
        r#"
ScriptName ChildScript Extends ParentScript

Function Takes(Int arg)
EndFunction

Function Test()
    Takes(self)
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects Int"));
}

#[test]
fn accepts_namespaced_self_against_same_namespace_spellings() {
    let diagnostics = check_fallout4(
        r#"
ScriptName MyMod:ChildScript Extends ParentScript

Function TakesSelfShort(ChildScript arg)
EndFunction

Function TakesSelfFull(mymod:childscript arg)
EndFunction

Function TakesParentFull(MyMod:ParentScript arg)
EndFunction

Function TakesParentShort(ParentScript arg)
EndFunction

Function Test()
    TakesSelfShort(self)
    TakesSelfFull(self)
    TakesParentFull(self)
    TakesParentShort(self)
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_namespaced_self_against_a_qualified_parent_spelled_short() {
    let diagnostics = check_fallout4(
        r#"
ScriptName MyMod:ChildScript Extends MyMod:ParentScript

Function Takes(ParentScript arg)
EndFunction

Function Test()
    Takes(self)
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_match_a_same_named_script_from_another_namespace() {
    let mut external = ChainExternal { complete: true };
    let source = r#"
ScriptName MyMod:ChainA Extends ChainB

Function Takes(OtherMod:ChainA arg)
EndFunction

Function Test()
    Takes(self)
EndFunction
"#;

    let diagnostics = check_fallout4(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects OtherMod:ChainA"));
}

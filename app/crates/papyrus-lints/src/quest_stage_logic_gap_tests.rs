use super::*;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

fn check_with(source: &str, external: &mut impl ExternalSignatures) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        external,
    )
}

fn quest(body: &str) -> String {
    format!("ScriptName MyQuest Extends Quest\n\nFunction Advance()\n{body}EndFunction\n")
}

#[test]
fn flags_an_unguarded_setstage() {
    let diagnostics = check(&quest("  SetStage(20)\n"));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[info]"));
}

#[test]
fn flags_self_setstage() {
    assert_eq!(check(&quest("  Self.SetStage(20)\n")).len(), 1);
}

#[test]
fn flags_in_events_and_states() {
    let source = "ScriptName MyQuest Extends Quest\n\nState Busy\n  Event OnInit()\n    SetStage(10)\n  EndEvent\nEndState\n";
    assert_eq!(check(source).len(), 1);
}

#[test]
fn does_not_flag_when_guarded_by_getstagedone() {
    assert!(check(&quest("  If !GetStageDone(20)\n    SetStage(20)\n  EndIf\n")).is_empty());
}

#[test]
fn does_not_flag_when_guarded_by_getstage() {
    assert!(check(&quest("  If GetStage() < 20\n    SetStage(20)\n  EndIf\n")).is_empty());
}

#[test]
fn does_not_flag_after_a_prior_stage_query() {
    assert!(check(&quest("  Bool done = IsStageDone(20)\n  SetStage(20)\n")).is_empty());
}

#[test]
fn a_query_after_the_setstage_does_not_guard_it() {
    assert_eq!(
        check(&quest("  SetStage(20)\n  If GetStageDone(20)\n  EndIf\n")).len(),
        1
    );
}

#[test]
fn ignores_setstage_on_other_quests() {
    assert!(check(&quest("  OtherQuest.SetStage(20)\n")).is_empty());
}

#[test]
fn ignores_non_quest_scripts() {
    let source =
        "ScriptName MyRef Extends ObjectReference\n\nFunction Advance()\n  SetStage(20)\nEndFunction\n";
    assert!(check(source).is_empty());
}

struct QuestChild;

impl ExternalSignatures for QuestChild {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case("MyQuestBase") && super_type.eq_ignore_ascii_case("Quest")
    }
}

#[test]
fn checks_scripts_extending_a_quest_subtype() {
    let source =
        "ScriptName MyQuest Extends MyQuestBase\n\nFunction Advance()\n  SetStage(20)\nEndFunction\n";
    assert_eq!(check_with(source, &mut QuestChild).len(), 1);
    assert!(check(source).is_empty());
}

#[test]
fn skips_scripts_with_fragment_markers() {
    let source = "ScriptName QF_MyQuest Extends Quest Hidden\n\n\
;BEGIN FRAGMENT Fragment_0\n\
Function Fragment_0()\n\
;BEGIN CODE\n\
SetStage(20)\n\
;END CODE\n\
EndFunction\n\
;END FRAGMENT\n";
    assert!(check(source).is_empty());
}

#[test]
fn skips_scripts_with_fragment_functions_even_without_markers() {
    let source = "ScriptName QF_MyQuest Extends Quest\n\n\
Function Fragment_3()\n  SetStage(30)\nEndFunction\n\n\
Function Helper()\n  SetStage(40)\nEndFunction\n";
    assert!(check(source).is_empty());
}

#[test]
fn a_function_merely_resembling_a_fragment_name_does_not_exempt() {
    let source = "ScriptName MyQuest Extends Quest\n\nFunction Fragment_Helper()\n  SetStage(30)\nEndFunction\n";
    assert_eq!(check(source).len(), 1);
}

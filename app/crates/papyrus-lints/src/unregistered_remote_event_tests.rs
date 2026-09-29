use super::*;
use crate::config::Config;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures};
use crate::Game;

fn parse_fo4(source: &str) -> papyrus_parser::ast::Script {
    papyrus_parser::parse_with_mode(source, papyrus_parser::parser::GameEdition::Fallout4)
        .expect("remote event should parse in Fallout 4 dialect")
}

fn check_with(source: &str, config: &Config) -> Vec<Diagnostic> {
    let ast = parse_fo4(source);
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        Some(&ast),
        tokens.as_deref(),
        config,
        &mut NoExternalSignatures,
    )
}

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &Config::default())
}

fn fo4() -> Config {
    Config {
        game: Game::Fallout4,
        ..Default::default()
    }
}

fn lint_fo4(source: &str, config: &Config) -> Vec<Diagnostic> {
    let ast = parse_fo4(source);
    papyrus_parser::prime_cache(source, ast);
    crate::lint(source, config)
}


#[test]
fn flags_remote_handler_without_registration() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("ObjectReference.OnCellAttach"));
    assert!(diagnostics[0].message.contains("RegisterForRemoteEvent"));
}

#[test]
fn does_not_flag_when_same_script_registers_matching_event() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_named_argument_matches() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start(ObjectReference akTarget)\n    RegisterForRemoteEvent(akSelf = akTarget, eventName = \"OnCellAttach\")\nEndFunction\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_registration_event_is_not_a_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start(ObjectReference akTarget, ScriptEventName eventName)\n    RegisterForRemoteEvent(akTarget, eventName)\nEndFunction\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_when_registered_event_name_does_not_match() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnDeath\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OnCellAttach"));
}

#[test]
fn flags_qualified_register_call_on_receiver() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    self.RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_ordinary_event() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit()\nEndEvent\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_unregister_as_registration() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    UnregisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nEvent OnInit()\n    registerforremoteevent(akTarget, \"oncellattach\")\nEndEvent\n\nEvent objectreference.oncellattach(ObjectReference akSender)\nEndEvent\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_remote_handler_inside_named_state() {
    let diagnostics = check(
        "ScriptName Example\n\nState Busy\n    Event ObjectReference.OnCellAttach(ObjectReference akSender)\n    EndEvent\nEndState\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn respects_disable_comment() {
    let source = "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender) ; @disable unregistered-remote-event\nEndEvent\n";
    assert_eq!(check(source).len(), 1);
    let filtered = lint_fo4(source, &fo4());
    assert!(filtered.iter().all(|d| d.rule != RULE));
}

#[test]
fn lint_runs_for_fallout4() {
    let diagnostics = lint_fo4(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &fo4(),
    );
    assert!(diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn lint_runs_for_starfield() {
    let config = Config {
        game: Game::Starfield,
        ..Default::default()
    };
    let diagnostics = lint_fo4(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &config,
    );
    assert!(diagnostics.iter().any(|d| d.rule == RULE));
}

#[test]
fn lint_skips_when_game_is_skyrim() {
    let diagnostics = crate::lint(
        "ScriptName Example\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n",
        &Config::default(),
    );
    assert!(diagnostics.iter().all(|d| d.rule != RULE));
}

struct AncestryRegistrations {
    /// Lowercased script name → (registered event leaves, opaque).
    scripts: std::collections::HashMap<String, (Vec<String>, bool)>,
    /// Lowercased script name → parent name (original case ok; looked up lower).
    extends: std::collections::HashMap<String, String>,
}

impl AncestryRegistrations {
    fn with_child_and_parent(
        parent: &str,
        parent_events: Vec<String>,
        parent_opaque: bool,
    ) -> Self {
        let mut scripts = std::collections::HashMap::new();
        scripts.insert("child".to_string(), (Vec::new(), false));
        scripts.insert(parent.to_ascii_lowercase(), (parent_events, parent_opaque));
        let mut extends = std::collections::HashMap::new();
        extends.insert("child".to_string(), parent.to_string());
        Self { scripts, extends }
    }

    fn parent_registers(parent: &str, event: &str) -> Self {
        Self::with_child_and_parent(parent, vec![event.to_ascii_lowercase()], false)
    }

    fn empty_parent(parent: &str) -> Self {
        Self::with_child_and_parent(parent, Vec::new(), false)
    }

    fn opaque_parent(parent: &str) -> Self {
        Self::with_child_and_parent(parent, Vec::new(), true)
    }
}

impl ExternalSignatures for AncestryRegistrations {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<crate::ParamInfo>> {
        None
    }

    fn registers_remote_event(&mut self, type_name: &str, event_name: &str) -> Option<bool> {
        let event_key = event_name.to_ascii_lowercase();
        let mut visited = Vec::new();
        let mut current = Some(type_name.to_ascii_lowercase());
        let mut saw_any = false;
        while let Some(name) = current {
            if visited.contains(&name) {
                return None;
            }
            let Some((events, opaque)) = self.scripts.get(&name) else {
                return None;
            };
            saw_any = true;
            if *opaque || events.iter().any(|event| event == &event_key) {
                return Some(true);
            }
            current = self.extends.get(&name).map(|parent| parent.to_ascii_lowercase());
            visited.push(name);
        }
        if saw_any {
            Some(false)
        } else {
            None
        }
    }
}

fn check_ancestry(source: &str, external: &mut AncestryRegistrations) -> Vec<Diagnostic> {
    let ast = parse_fo4(source);
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        Some(&ast),
        tokens.as_deref(),
        &Config::default(),
        external,
    )
}

const CHILD_HANDLER: &str = "ScriptName Child Extends ParentScript\n\nEvent ObjectReference.OnCellAttach(ObjectReference akSender)\nEndEvent\n";

#[test]
fn does_not_flag_when_parent_registers_matching_event() {
    let mut external = AncestryRegistrations::parent_registers("ParentScript", "OnCellAttach");
    assert!(check_ancestry(CHILD_HANDLER, &mut external).is_empty());
}

#[test]
fn flags_when_neither_self_nor_parent_registers() {
    let mut external = AncestryRegistrations::empty_parent("ParentScript");
    let diagnostics = check_ancestry(CHILD_HANDLER, &mut external);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("ObjectReference.OnCellAttach"));
    assert!(diagnostics[0].message.contains("parent it Extends"));
}

#[test]
fn does_not_flag_when_parent_has_opaque_registration() {
    let mut external = AncestryRegistrations::opaque_parent("ParentScript");
    assert!(check_ancestry(CHILD_HANDLER, &mut external).is_empty());
}

#[test]
fn does_not_flag_extends_script_when_ancestry_cannot_be_resolved() {
    // NoExternalSignatures → registers_remote_event is None; Extends present → quiet.
    let diagnostics = check(CHILD_HANDLER);
    assert!(diagnostics.is_empty());
}

#[test]
fn remote_event_registrations_collects_literals_and_opaque() {
    let literal = parse_fo4(
        "ScriptName Example\n\nEvent OnInit()\n    RegisterForRemoteEvent(akTarget, \"OnCellAttach\")\nEndEvent\n",
    );
    let regs = remote_event_registrations(&literal);
    assert!(regs.events.contains("oncellattach"));
    assert!(!regs.opaque);

    let opaque = parse_fo4(
        "ScriptName Example\n\nFunction Start(ScriptEventName eventName)\n    RegisterForRemoteEvent(akTarget, eventName)\nEndFunction\n",
    );
    let regs = remote_event_registrations(&opaque);
    assert!(regs.events.is_empty());
    assert!(regs.opaque);
}

//! Regression test for https://github.com/Idrinth/papyrus-lint/issues/1082:
//! legal vanilla upcasts (`Actor` → `ObjectReference`) must stay silent
//! even when the project ships none of the game's own `.psc` files.
//!
//! `VahlokInheritProbe.psc` is the issue's repro. Resolution has to come
//! from the bundled Skyrim AST cache by `ScriptName`, not from a native
//! YAML fallback and not from files on disk.

use papyrus_lint_core::function_table::FunctionTable;

const SOURCE: &str = include_str!("fixtures/VahlokInheritProbe.psc");

fn type_diagnostics(rule: &str) -> Vec<papyrus_lints::Diagnostic> {
    let empty_root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(empty_root.path().to_path_buf());
    papyrus_lints::lint_with_external_arguments(
        SOURCE,
        &papyrus_lints::Config::default(),
        &mut table,
    )
    .into_iter()
    .filter(|diagnostic| diagnostic.rule == rule)
    .collect()
}

#[test]
fn accepts_legal_vanilla_upcasts_without_game_scripts_on_disk() {
    let arguments = type_diagnostics("argument-types");
    let returns = type_diagnostics("return-types");

    assert_eq!(
        arguments.len(),
        2,
        "expected downcast + sibling argument mismatches, got {arguments:?}"
    );
    assert!(
        arguments.iter().any(|d| d.message.contains("got Form")),
        "expected Form downcast, got {arguments:?}"
    );
    assert!(
        arguments.iter().any(|d| d.message.contains("got Spell")),
        "expected Spell sibling, got {arguments:?}"
    );
    assert!(
        arguments.iter().all(|d| !d.message.contains("got Actor")),
        "Actor → ObjectReference is a legal upcast, got {arguments:?}"
    );

    assert_eq!(
        returns.len(),
        1,
        "expected one sibling return mismatch, got {returns:?}"
    );
    assert!(
        returns[0].message.contains("returns Spell"),
        "expected Spell sibling return, got {returns:?}"
    );
    assert!(
        returns.iter().all(|d| !d.message.contains("returns Actor")),
        "returning Actor from ObjectReference is a legal upcast, got {returns:?}"
    );
}

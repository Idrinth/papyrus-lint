//! Flag-gate coverage for `register_for_update_in_on_update`.

use super::super::*;
use super::support::config_with;

#[test]
fn register_for_update_in_on_update_flag_gates_lint() {
    let source = "ScriptName Example\n\nEvent OnUpdate()\n    RegisterForUpdate(1.0)\nEndEvent\n";
    let baseline = lint(source, &Config::default());
    assert!(
        baseline
            .iter()
            .any(|d| d.rule == register_for_update_in_on_update::RULE),
        "expected rule to fire, got {baseline:?}"
    );
    let disabled = lint(
        source,
        &config_with(|c| c.rules.register_for_update_in_on_update = false),
    );
    assert!(
        disabled
            .iter()
            .all(|d| d.rule != register_for_update_in_on_update::RULE),
        "disabling should suppress diagnostics, got {disabled:?}"
    );
}

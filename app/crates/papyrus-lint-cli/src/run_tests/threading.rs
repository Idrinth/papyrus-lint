//! Threaded vs. sequential runs must report identical results.

use crate::test_support::*;

// Builds an achlist with enough scripts, several of them cross-referencing
// a shared base script, that a multi-threaded run actually exercises
// more than one worker thread and more than one `SharedFunctionTable`
// lookup collision -- then checks a `--threads 1` (fully sequential) run
// and the default multi-threaded run agree byte-for-byte, since threading
// is only ever meant to change how fast a run finishes, never what it
// reports or in what order.
#[test]
fn threaded_and_sequential_runs_report_identical_results() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Base.psc"),
        "ScriptName Base\n\nFunction DoThing(int arg1)\nEndFunction\n",
    );
    let mut entries = vec!["\"scripts/source/Base.psc\"".to_string()];
    for i in 0..12 {
        let name = format!("Child{i}");
        write_file(
            &dir.path().join(format!("scripts/source/{name}.psc")),
            &format!(
                "ScriptName {name} Extends Base\n\nFunction UseIt()\n    DoThing(\"wrong type\")   \nEndFunction\n"
            ),
        );
        entries.push(format!("\"scripts/source/{name}.psc\""));
    }
    write_file(
        &dir.path().join("sources.achlist"),
        &format!("[{}]", entries.join(", ")),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (sequential_code, sequential_stdout, _) = run_captured(&[
        "--threads=1".to_string(),
        "--short-paths".to_string(),
        "--format=json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);
    let (parallel_code, parallel_stdout, _) = run_captured(&[
        "--threads=8".to_string(),
        "--short-paths".to_string(),
        "--format=json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(sequential_code, parallel_code);
    assert_eq!(sequential_stdout, parallel_stdout);
    // Sanity check that this fixture actually triggers diagnostics
    // (the argument-type mismatch on every child script), rather than
    // both runs trivially agreeing on an empty report.
    assert!(sequential_stdout.contains("argument-types"));
}

use super::super::*;
use tempfile::tempdir;

#[test]
fn lint_psc_file_lints_source_from_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "forbidden-functions"));
}

#[test]
fn lint_psc_file_reports_a_source_read_error() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("Missing.psc");

    let error = lint_psc_file(
        missing.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap_err();

    assert!(!error.is_empty());
}

#[test]
fn lint_psc_file_applies_project_ignore_entries() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();
    std::fs::write(
        dir.path()
            .join(papyrus_lint_core::ignore_file::IGNORE_FILE_NAME),
        "- file: Example.psc\n  line: 4\n  rule: forbidden-functions\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "forbidden-functions"));
}

#[test]
fn preload_project_scripts_lets_lint_psc_file_resolve_a_sibling_immediately() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let base_path = source_dir.join("BaseScript.psc");
    std::fs::write(
        &base_path,
        "ScriptName BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();
    let derived_path = source_dir.join("DerivedScript.psc");
    std::fs::write(
        &derived_path,
        "ScriptName DerivedScript extends BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();

    let context = ProjectLintContext {
        root: dir.path().to_string_lossy().into_owned(),
        ..Default::default()
    };

    preload_project_scripts(
        vec![
            base_path.to_string_lossy().into_owned(),
            derived_path.to_string_lossy().into_owned(),
        ],
        context.clone(),
        None.into(),
    );

    let diagnostics = lint_psc_file(derived_path.to_string_lossy().into_owned(), context).unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "function-override"));
}

#[test]
fn lint_psc_file_resolves_argument_types_through_additional_script_roots() {
    let extra = tempdir().unwrap();
    std::fs::write(
        extra.path().join("Helpers.psc"),
        "ScriptName Helpers\n\nFunction NeedInt(Int count)\nEndFunction\n",
    )
    .unwrap();
    let dir = tempdir().unwrap();
    let path = dir.path().join("Caller.psc");
    std::fs::write(
            &path,
            "ScriptName Caller\n\nFunction Run(Helpers helper)\n    helper.NeedInt(\"nope\")\nEndFunction\n",
        )
        .unwrap();
    let extra_root = extra.path().to_string_lossy().into_owned();

    let without_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(without_roots
        .iter()
        .all(|diagnostic| diagnostic.rule != "argument-types"));

    let with_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            additional_roots: vec![extra_root],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(with_roots.iter().any(|diagnostic| {
        diagnostic.rule == "argument-types"
            && diagnostic.message.contains("expects Int")
            && diagnostic.message.contains("got String")
    }));
}

#[test]
fn preload_project_scripts_closes_over_a_parent_that_was_not_in_the_batch() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let base_path = source_dir.join("BaseScript.psc");
    std::fs::write(
        &base_path,
        "ScriptName BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();
    let derived_path = source_dir.join("DerivedScript.psc");
    std::fs::write(
        &derived_path,
        "ScriptName DerivedScript extends BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();

    let context = ProjectLintContext {
        root: dir.path().to_string_lossy().into_owned(),
        ..Default::default()
    };

    preload_project_scripts(
        vec![derived_path.to_string_lossy().into_owned()],
        context.clone(),
        None.into(),
    );

    let diagnostics = lint_psc_file(derived_path.to_string_lossy().into_owned(), context).unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "function-override"));
}

#[test]
fn preload_project_scripts_reports_parsing_progress_including_parents_outside_the_batch() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("BaseScript.psc"),
        "ScriptName BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();
    let derived_path = source_dir.join("DerivedScript.psc");
    std::fs::write(
        &derived_path,
        "ScriptName DerivedScript extends BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = std::sync::Arc::clone(&seen);
    let channel = tauri::ipc::Channel::new(move |body| {
        let tauri::ipc::InvokeResponseBody::Json(json) = body else {
            return Ok(());
        };
        captured.lock().unwrap().push(json);
        Ok(())
    });

    preload_project_scripts(
        vec![derived_path.to_string_lossy().into_owned()],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        Some(channel).into(),
    );

    let seen = seen.lock().unwrap();
    let parsing: Vec<(u64, u64)> = seen
        .iter()
        .filter_map(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .filter(|progress| progress["phase"] == "Parsing")
        .map(|progress| {
            (
                progress["completed"].as_u64().unwrap_or(0),
                progress["total"].as_u64().unwrap_or(0),
            )
        })
        .collect();
    assert!(
        parsing.len() >= 2,
        "expected the seed and its parent to report progress, got {parsing:?}"
    );
    assert!(
        parsing.iter().any(|(_, total)| *total >= 2),
        "expected the reported total to grow past the single lint target, got {parsing:?}"
    );
    assert!(seen.iter().any(|json| json.contains("Indexing scripts")));
}

fn project_lint_events(paths: Vec<String>, context: ProjectLintContext) -> Vec<serde_json::Value> {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = std::sync::Arc::clone(&seen);
    let channel = tauri::ipc::Channel::new(move |body| {
        let tauri::ipc::InvokeResponseBody::Json(json) = body else {
            return Ok(());
        };
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&json) {
            captured.lock().unwrap().push(event);
        }
        Ok(())
    });
    lint_project_scripts(paths, context, Some(channel).into());
    let events = seen.lock().unwrap().clone();
    events
}

#[test]
fn lint_project_scripts_resolves_a_sibling_and_reports_its_findings() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let base_path = source_dir.join("BaseScript.psc");
    std::fs::write(
        &base_path,
        "ScriptName BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();
    let derived_path = source_dir.join("DerivedScript.psc");
    std::fs::write(
        &derived_path,
        "ScriptName DerivedScript extends BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();

    let events = project_lint_events(
        vec![
            base_path.to_string_lossy().into_owned(),
            derived_path.to_string_lossy().into_owned(),
        ],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let derived = events.iter().find(|event| {
        event["kind"] == "result"
            && event["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("DerivedScript.psc"))
    });
    let derived = derived.expect("derived script result");
    assert_eq!(derived["ok"], true);
    assert_eq!(derived["detail"], "parsed as \"DerivedScript\"");
    let findings = derived["findings"].as_array().expect("findings");
    assert!(findings
        .iter()
        .any(|finding| finding["rule"] == "function-override"));
    assert!(events.iter().any(|event| {
        event["kind"] == "progress" && event["phase"] == "Indexing scripts" && event["total"] == 0
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "progress"
            && event["phase"] == "Linting"
            && event["completed"] == 2
            && event["total"] == 2
    }));
}

#[test]
fn lint_project_scripts_reports_an_unparseable_file_without_linting_it() {
    let dir = tempdir().unwrap();
    let broken_path = dir.path().join("Broken.psc");
    std::fs::write(&broken_path, "this is not papyrus\n").unwrap();
    let ok_path = dir.path().join("Example.psc");
    std::fs::write(
        &ok_path,
        "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let events = project_lint_events(
        vec![
            broken_path.to_string_lossy().into_owned(),
            ok_path.to_string_lossy().into_owned(),
        ],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let broken = events
        .iter()
        .find(|event| event["kind"] == "result" && event["ok"] == false)
        .expect("broken script result");
    assert_eq!(broken["findings"].as_array().map(Vec::len), Some(0));
    assert!(
        !broken["detail"].as_str().unwrap_or("").is_empty(),
        "parse failure should explain itself"
    );
    let example = events.iter().find(|event| {
        event["kind"] == "result"
            && event["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("Example.psc"))
    });
    let findings = example.expect("example result")["findings"]
        .as_array()
        .expect("findings");
    assert!(findings
        .iter()
        .any(|finding| finding["rule"] == "forbidden-functions"));
}

#[test]
fn lint_project_scripts_reports_missing_files() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("Missing.psc");

    let events = project_lint_events(
        vec![missing.to_string_lossy().into_owned()],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let result = events
        .iter()
        .find(|event| event["kind"] == "result")
        .expect("missing script result");
    assert_eq!(result["ok"], false);
    assert!(result["detail"]
        .as_str()
        .is_some_and(|detail| !detail.is_empty()));
    assert_eq!(result["findings"].as_array().map(Vec::len), Some(0));
}

#[test]
fn lint_project_scripts_reports_an_invalid_ignore_file_for_every_path() {
    let dir = tempdir().unwrap();
    let first = dir.path().join("First.psc");
    let second = dir.path().join("Second.psc");
    std::fs::write(&first, "ScriptName First\n").unwrap();
    std::fs::write(&second, "ScriptName Second\n").unwrap();
    std::fs::write(
        dir.path()
            .join(papyrus_lint_core::ignore_file::IGNORE_FILE_NAME),
        "- file: First.psc\n  line: 0\n  rule: semicolon\n",
    )
    .unwrap();

    let events = project_lint_events(
        vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let results: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "result")
        .collect();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|event| {
        event["ok"] == false
            && event["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("line numbers must be 1 or greater"))
    }));
    assert!(events.iter().any(|event| {
        event["kind"] == "progress"
            && event["phase"] == "Linting"
            && event["completed"] == 2
            && event["total"] == 2
    }));
}

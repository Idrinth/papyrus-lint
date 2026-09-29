use super::super::*;
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tempfile::tempdir;

fn invoke_request(command: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: command.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
fn batch_commands_accept_a_null_progress_channel_over_ipc() {
    let app = crate::configure_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "batch-null-channel", Default::default())
        .build()
        .unwrap();
    let context = serde_json::to_value(ProjectLintContext::default()).unwrap();

    for command in ["preload_project_scripts", "lint_project_scripts"] {
        get_ipc_response(
            &webview,
            invoke_request(
                command,
                serde_json::json!({
                    "paths": [],
                    "context": context,
                    "on_progress": null
                }),
            ),
        )
        .unwrap_or_else(|error| panic!("{command} rejected a null channel: {error}"));
    }
}

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
fn lint_psc_file_reports_an_invalid_project_ignore_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    std::fs::write(
        dir.path()
            .join(papyrus_lint_core::ignore_file::IGNORE_FILE_NAME),
        "- file: Example.psc\n  line: 0\n  rule: semicolon\n",
    )
    .unwrap();

    let error = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap_err();

    assert!(error.contains("line numbers must be 1 or greater"));
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
fn strict_scope_function_table_resolves_only_listed_scripts() {
    let root = tempdir().unwrap();
    let external = tempdir().unwrap();
    let listed = external.path().join("Listed.psc");
    let unlisted = external.path().join("Unlisted.psc");
    std::fs::write(&listed, "ScriptName Listed\n").unwrap();
    std::fs::write(&unlisted, "ScriptName Unlisted\n").unwrap();
    let context = ProjectLintContext {
        root: root.path().to_string_lossy().into_owned(),
        strict_achlist_scope: true,
        known_scripts: vec![listed.to_string_lossy().into_owned()],
        ..Default::default()
    };

    let table = context.function_table();
    let table = table.read().unwrap();
    assert!(table.script_exists("Listed"));
    assert!(!table.script_exists("Unlisted"));
}

#[test]
fn lint_project_scripts_applies_project_ignore_entries() {
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

    let events = project_lint_events(
        vec![path.to_string_lossy().into_owned()],
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let result = events
        .iter()
        .find(|event| event["kind"] == "result")
        .expect("script result");
    assert_eq!(result["ok"], true);
    assert!(result["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .all(|finding| finding["rule"] != "forbidden-functions"));
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

#[test]
fn lint_preloaded_script_uses_fallback_parse_error_details() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Broken.psc");
    let function_table = project_function_table(
        dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );
    let outcome = lint_preloaded_script(
        "display/Broken.psc",
        &path,
        &ProjectScriptParse {
            source: Some("not papyrus".to_string()),
            ast: None,
            tokens: None,
            error: None,
        },
        &ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        &function_table,
        None,
    );

    assert!(!outcome.ok);
    assert_eq!(outcome.path, "display/Broken.psc");
    assert_eq!(outcome.detail, "failed to parse script");
    assert!(outcome.findings.is_empty());
}

#[test]
fn lint_preloaded_script_uses_fallback_read_error_details() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Missing.psc");
    let function_table = project_function_table(
        dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );
    let ast = papyrus_parser::parse("ScriptName Missing\n").unwrap();
    let outcome = lint_preloaded_script(
        "display/Missing.psc",
        &path,
        &ProjectScriptParse {
            source: None,
            ast: Some(ast),
            tokens: None,
            error: None,
        },
        &ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        &function_table,
        None,
    );

    assert!(!outcome.ok);
    assert_eq!(outcome.path, "display/Missing.psc");
    assert_eq!(outcome.detail, "failed to read script");
    assert!(outcome.findings.is_empty());
}

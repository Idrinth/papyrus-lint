use std::fs;

use super::*;
use crate::script_locator::ScriptIndex;
use papyrus_lints::NoExternalSignatures;

fn config_with(edit: impl FnOnce(&mut papyrus_lints::Config)) -> papyrus_lints::Config {
    let mut config = papyrus_lints::Config::default();
    edit(&mut config);
    config
}

fn lint_at(
    path: &Path,
    source: &str,
    config: &papyrus_lints::Config,
    conflicts: ConflictScope<'_>,
    ignores: Option<&IgnoreFile>,
) -> Vec<Diagnostic> {
    lint_script(
        path,
        source,
        &mut NoExternalSignatures,
        &ProjectLint {
            config,
            project_root: path.parent().unwrap(),
            additional_roots: &[],
            conflicts,
            compile_check: true,
            compiler_path: "   ",
            ignores,
            already_primed: false,
            flush_collision_cache: false,
        },
    )
}

#[test]
fn merges_a_filename_mismatch_and_counts_its_disable_as_used() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Other ; @disable script-filename-mismatch\n";
    fs::write(&path, source).expect("write script");
    let config = config_with(|config| {
        config.rules.script_filename_mismatch = true;
        config.rules.unused_disable = true;
        config.rules.conflicting_script_versions = false;
        config.rules.stale_compiled_output = false;
    });

    let diagnostics = lint_at(
        &path,
        source,
        &config,
        ConflictScope::Known {
            candidates: &[],
            short_paths: false,
        },
        None,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "script-filename-mismatch"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn drops_a_diagnostic_named_by_the_project_ignore_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n";
    fs::write(&path, source).expect("write script");
    fs::write(
        dir.path().join(crate::ignore_file::IGNORE_FILE_NAME),
        "- file: Example.psc\n  line: 4\n  rule: forbidden-functions\n",
    )
    .expect("write ignore file");
    let ignores = IgnoreFile::load_optional(dir.path())
        .expect("load ignore file")
        .expect("ignore file exists");
    let config = config_with(|config| {
        config.rules.conflicting_script_versions = false;
        config.rules.stale_compiled_output = false;
        config.rules.script_filename_mismatch = false;
    });

    let diagnostics = lint_at(
        &path,
        source,
        &config,
        ConflictScope::Index {
            index: &ScriptIndex::new(),
            short_paths: false,
        },
        Some(&ignores),
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "forbidden-functions"));
}

#[test]
fn known_candidates_and_an_index_report_the_same_conflict() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("a").join("Example.psc");
    let second = dir.path().join("b").join("Example.psc");
    fs::create_dir_all(first.parent().unwrap()).expect("dir a");
    fs::create_dir_all(second.parent().unwrap()).expect("dir b");
    fs::write(&first, "ScriptName Example\n").expect("write first");
    fs::write(&second, "ScriptName Example\n; other\n").expect("write second");
    let config = config_with(|config| {
        config.rules.conflicting_script_versions = true;
        config.rules.stale_compiled_output = false;
        config.rules.script_filename_mismatch = false;
    });
    let known = vec![first.clone(), second.clone()];
    let mut index = ScriptIndex::new();
    index.insert("example.psc".to_string(), known.clone());

    let from_known = lint_at(
        &first,
        "ScriptName Example\n",
        &config,
        ConflictScope::Known {
            candidates: &known,
            short_paths: false,
        },
        None,
    );
    let from_index = lint_at(
        &first,
        "ScriptName Example\n",
        &config,
        ConflictScope::Index {
            index: &index,
            short_paths: false,
        },
        None,
    );

    let conflict = |diagnostics: &[Diagnostic]| {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "conflicting-script-versions")
    };
    assert!(conflict(&from_known));
    assert!(conflict(&from_index));
}

#[test]
fn a_blank_compiler_path_does_not_fail_the_lint() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    fs::write(&path, source).expect("write script");
    let config = config_with(|config| {
        config.rules.conflicting_script_versions = false;
        config.rules.stale_compiled_output = false;
        config.rules.script_filename_mismatch = false;
    });

    let diagnostics = lint_at(
        &path,
        source,
        &config,
        ConflictScope::Known {
            candidates: &[],
            short_paths: true,
        },
        None,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "compiler-error"));
}

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::*;
use papyrus_parser::ast::Script;

struct Seed {
    source: Option<String>,
    ast: Option<Script>,
    tokens: Option<Vec<papyrus_parser::token::Token>>,
}

fn write_script(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let source_dir = dir.join("scripts/source");
    fs::create_dir_all(&source_dir).expect("source dir");
    let path = source_dir.join(format!("{name}.psc"));
    fs::write(&path, contents).expect("write script");
    path
}

fn read_seed(path: &Path) -> Seed {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(_) => {
            return Seed {
                source: None,
                ast: None,
                tokens: None,
            };
        }
    };
    let ast = papyrus_parser::parse(&source).ok();
    let tokens = papyrus_parser::tokenize(&source).ok();
    Seed {
        source: Some(source),
        ast,
        tokens,
    }
}

fn seed_script(seed: &Seed) -> Option<SeedScript<'_>> {
    Some(SeedScript {
        source: seed.source.as_deref()?,
        ast: seed.ast.as_ref(),
    })
}

fn parsed_memo(seed: &Seed) -> Option<ParsedMemo<'_>> {
    Some(ParsedMemo {
        source: seed.source.as_deref()?,
        ast: seed.ast.as_ref(),
        tokens: seed.tokens.as_deref(),
    })
}

#[test]
fn closure_grows_the_total_and_keeps_seed_order() {
    let root = tempfile::tempdir().expect("temp dir");
    let _base = write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromBase()\nEndFunction\n",
    );
    let child = write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );
    let other = write_script(root.path(), "Other", "ScriptName Other\n");
    let table = FunctionTable::new(root.path().to_path_buf());
    let finished = AtomicUsize::new(0);
    let saw_grown_total = AtomicUsize::new(0);
    let seeds = [child, other];
    let closed = parse_closure(
        &table,
        &seeds,
        ClosureOptions {
            threads: 4,
            track_total: true,
        },
        read_seed,
        |seed| seed.ast.as_ref(),
        |progress| {
            finished.fetch_add(1, Ordering::SeqCst);
            if progress.total > 2 {
                saw_grown_total.fetch_add(1, Ordering::SeqCst);
            }
            assert!(progress.completed <= progress.total);
        },
    );

    assert_eq!(closed.seeds.len(), 2);
    assert_eq!(
        closed.seeds[0].ast.as_ref().map(|ast| ast.name.as_str()),
        Some("Child")
    );
    assert_eq!(
        closed.seeds[1].ast.as_ref().map(|ast| ast.name.as_str()),
        Some("Other")
    );
    assert!(closed
        .dependencies
        .iter()
        .any(|dependency| dependency.name_lower == "basescript"));
    assert!(finished.load(Ordering::SeqCst) >= 3);
    assert!(saw_grown_total.load(Ordering::SeqCst) > 0);
}

#[test]
fn preload_keeps_a_dependency_when_the_file_later_changes_under_the_same_mtime() {
    let root = tempfile::tempdir().expect("temp dir");
    let child = write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n\nFunction Run()\n    BaseScript.FromBase()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromBase()\nEndFunction\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let closed = parse_closure(
        &table,
        std::slice::from_ref(&child),
        ClosureOptions {
            threads: 1,
            track_total: false,
        },
        read_seed,
        |seed| seed.ast.as_ref(),
        |_| {},
    );
    let parent = root.path().join("scripts/source/BaseScript.psc");
    let mtime = fs::metadata(&parent)
        .expect("parent metadata")
        .modified()
        .expect("parent mtime");
    preload_closure(
        &mut table,
        std::slice::from_ref(&child),
        &closed,
        seed_script,
    );

    fs::write(
        &parent,
        "ScriptName BaseScript\n\nFunction Replaced()\nEndFunction\n",
    )
    .expect("overwrite parent");
    let file = fs::File::open(&parent).expect("reopen parent");
    if file.set_modified(mtime).is_err() {
        return;
    }
    drop(file);

    assert!(table.lookup_function("BaseScript", "FromBase").is_some());
    assert!(table.lookup_function("BaseScript", "Replaced").is_none());
    assert!(table.lookup_function("Child", "Run").is_some());
}

#[test]
fn preload_records_an_unresolved_name_and_skips_an_unreadable_seed() {
    let root = tempfile::tempdir().expect("temp dir");
    let child = write_script(
        root.path(),
        "Child",
        "ScriptName Child\n\nFunction Run()\n    MissingScript.DoIt()\nEndFunction\n",
    );
    let missing = root.path().join("scripts/source/Gone.psc");
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let paths = [child, missing];
    let closed = parse_closure(
        &table,
        &paths,
        ClosureOptions {
            threads: 1,
            track_total: true,
        },
        read_seed,
        |seed| seed.ast.as_ref(),
        |_| {},
    );
    assert!(closed.seeds[1].source.is_none());
    assert!(closed.unresolved.iter().any(|name| name == "missingscript"));
    preload_closure(&mut table, &paths, &closed, seed_script);
    assert!(table.lookup_function("MissingScript", "DoIt").is_none());
    assert!(table.lookup_function("Gone", "Anything").is_none());
    assert!(table.lookup_function("Child", "Run").is_some());
}

#[test]
fn parallel_lint_primes_before_the_callback_and_keeps_order() {
    let source = "ScriptName Example\n";
    let planted = papyrus_parser::parse("ScriptName Planted\n").expect("planted ast");
    let seeds = vec![Seed {
        source: Some(source.to_string()),
        ast: Some(planted),
        tokens: Some(Vec::new()),
    }];
    let names = lint_in_parallel(&seeds, 1, parsed_memo, |index, seed| {
        assert_eq!(index, 0);
        papyrus_parser::parse(seed.source.as_deref().unwrap())
            .expect("primed parse")
            .name
    });
    assert_eq!(names, vec!["Planted".to_string()]);

    let values: Vec<usize> = (0..8).collect();
    let ordered = lint_in_parallel(
        &values,
        4,
        |_| None,
        |index, value| {
            assert_eq!(index, *value);
            *value
        },
    );
    assert_eq!(ordered, values);
}

#[test]
fn empty_tokens_are_not_primed() {
    let source = "ScriptName Example\n";
    prime_parsed_memo(ParsedMemo {
        source,
        ast: None,
        tokens: Some(&[]),
    });
    let tokens = papyrus_parser::tokenize(source).expect("tokens");
    assert!(!tokens.is_empty());
}

#[test]
fn parse_progress_is_quiet_when_nothing_is_listening_for_an_empty_batch() {
    let root = tempfile::tempdir().expect("temp dir");
    let table = FunctionTable::new(root.path().to_path_buf());
    let calls = Mutex::new(0);
    let closed = parse_closure(
        &table,
        &[],
        ClosureOptions {
            threads: 2,
            track_total: true,
        },
        read_seed,
        |seed: &Seed| seed.ast.as_ref(),
        |_| {
            *calls.lock().expect("calls") += 1;
        },
    );
    assert!(closed.seeds.is_empty());
    assert_eq!(*calls.lock().expect("calls"), 0);
}

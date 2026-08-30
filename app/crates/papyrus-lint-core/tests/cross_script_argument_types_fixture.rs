//! Integration test using real `.psc` fixture files (rather than inline
//! source strings) to make sure the "Argument type check" lint resolves a
//! self call to a function declared on a *parent* script through the
//! `FunctionTable`'s `Extends` chain resolution, accepting an argument
//! whose own type is a subtype of the parent function's declared parameter
//! type.
//!
//! `typeb.psc` (`TypeB Extends TypeA`) calls `A(ab)` with no explicit
//! receiver from inside its own `Function B`; `A` is only declared on
//! `TypeA`, and its `aa` parameter is typed `TypeA` while the argument
//! `ab` is typed `TypeB`. Resolving this correctly requires the lint to:
//! walk up from `TypeB`'s own (self) type to find `A` declared on its
//! parent `TypeA`, and recognize `TypeB` as a subtype of `TypeA` for the
//! argument itself.

use std::path::PathBuf;

use papyrus_lint_core::function_table::FunctionTable;

const TYPEB: &str = include_str!("fixtures/typeb.psc");

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn resolves_a_self_call_to_a_parent_scripts_function_through_the_extends_chain() {
    let empty_root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new_with_additional_roots(
        empty_root.path().to_path_buf(),
        vec![fixtures_dir().to_string_lossy().into_owned()],
    );

    let diagnostics = papyrus_lints::argument_types::check_with(TYPEB, &mut table);

    assert!(
        diagnostics.is_empty(),
        "expected no argument-type diagnostics, got {diagnostics:?}"
    );
}

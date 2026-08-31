//! Integration test using real `.psc` fixture files (rather than inline
//! source strings) to make sure the "Return type check" lint resolves
//! subtype relationships that mix project scripts with a native engine
//! type in the middle of the `Extends` chain.
//!
//! `typec.psc` (`TypeC Extends Armor`, `Armor` a native engine type with no
//! `.psc` in the project) and `typed.psc` (`TypeD Extends TypeC`) exercise:
//! - `TypeC.GetMe` (declared `Armor Function`) returning `self` (`TypeC`),
//!   a direct project-to-native subtype;
//! - `TypeC.GetC` (declared `Form Function`) returning its `TypeC`-typed
//!   `c` property, requiring the chain `TypeC -> Armor -> Form` to resolve
//!   through the native fallback past `Armor`;
//! - `TypeD.GetAsA` (declared `TypeC Function`) returning `self` (`TypeD`),
//!   a direct project-to-project subtype;
//! - `TypeD.GetD` (declared `Armor Function`) returning its `TypeD`-typed
//!   `d` property, requiring the transitive chain `TypeD -> TypeC -> Armor`
//!   across two project scripts plus the native fallback past `Armor`.

use std::path::PathBuf;

use papyrus_lint_core::function_table::FunctionTable;

const TYPEC: &str = include_str!("fixtures/typec.psc");
const TYPED: &str = include_str!("fixtures/typed.psc");

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn table() -> FunctionTable {
    let empty_root = tempfile::tempdir().expect("failed to create temp dir");
    FunctionTable::new_with_additional_roots(
        empty_root.path().to_path_buf(),
        vec![fixtures_dir().to_string_lossy().into_owned()],
    )
}

#[test]
fn accepts_returns_through_a_project_and_native_extends_chain() {
    let mut table = table();

    let diagnostics = papyrus_lints::return_types::check_with(TYPEC, &mut table);

    assert!(
        diagnostics.is_empty(),
        "expected no return-type diagnostics for typec.psc, got {diagnostics:?}"
    );
}

#[test]
fn accepts_returns_through_a_transitive_project_and_native_extends_chain() {
    let mut table = table();

    let diagnostics = papyrus_lints::return_types::check_with(TYPED, &mut table);

    assert!(
        diagnostics.is_empty(),
        "expected no return-type diagnostics for typed.psc, got {diagnostics:?}"
    );
}

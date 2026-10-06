//! The "Argument type check" lint must accept `self` for a parameter typed
//! as any `Extends` ancestor of the script being linted, resolved through a
//! real [`FunctionTable`], even when that script itself is not on disk (an
//! unsaved editor buffer, or a file outside the configured roots). The
//! table can't walk an `Extends` chain from a name it can't find, so the
//! lint starts the walk from the script's declared parent instead.

use std::fs;
use std::path::Path;

use papyrus_lint_core::function_table::FunctionTable;

fn write_script(dir: &Path, file_name: &str, source: &str) {
    if let Err(error) = fs::write(dir.join(file_name), source) {
        panic!("failed to write fixture script {file_name}: {error}");
    }
}

const BASE: &str = "ScriptName SelfArgBase\n";
const MID: &str = "ScriptName SelfArgMid Extends SelfArgBase\n";
const LEAF: &str = "ScriptName SelfArgLeaf Extends SelfArgMid\n";
const UTIL: &str = concat!(
    "ScriptName SelfArgUtil\n\n",
    "Function TakesBase(SelfArgBase arg)\n",
    "EndFunction\n\n",
    "Function TakesMid(SelfArgMid arg)\n",
    "EndFunction\n\n",
    "Function TakesLeaf(SelfArgLeaf arg)\n",
    "EndFunction\n",
);

const ALL_SCRIPTS: [&str; 4] = ["SelfArgBase", "SelfArgMid", "SelfArgLeaf", "SelfArgUtil"];

/// `SelfArgMid Extends SelfArgBase`, `SelfArgLeaf Extends SelfArgMid`, and
/// `SelfArgUtil` declaring one function per type. Only the scripts named
/// in `on_disk` are written.
fn project(on_disk: &[&str]) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let scripts = [
        ("SelfArgBase", BASE),
        ("SelfArgMid", MID),
        ("SelfArgLeaf", LEAF),
        ("SelfArgUtil", UTIL),
    ];
    for (name, source) in scripts {
        if on_disk.contains(&name) {
            write_script(root.path(), &format!("{name}.psc"), source);
        }
    }
    root
}

/// A table that finds `root`'s scripts by name, the way a configured script
/// root (`additional_roots`) is searched.
fn table_for(root: &tempfile::TempDir) -> FunctionTable {
    FunctionTable::new_with_additional_roots(
        root.path().to_path_buf(),
        vec![root.path().to_string_lossy().into_owned()],
    )
}

#[test]
fn accepts_self_for_parent_and_grandparent_parameters_of_an_unsaved_script() {
    let root = project(&["SelfArgBase", "SelfArgMid", "SelfArgUtil"]);
    let mut table = table_for(&root);
    let source = concat!(
        "ScriptName SelfArgLeaf Extends SelfArgMid\n\n",
        "SelfArgUtil Property Util Auto\n\n",
        "Function Test()\n",
        "    Util.TakesMid(self)\n",
        "    Util.TakesBase(self)\n",
        "    Util.TakesBase(Self)\n",
        "EndFunction\n",
    );

    let diagnostics = papyrus_lints::check_argument_types(source, &mut table);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_self_for_a_grandparent_parameter_of_a_script_on_disk() {
    let root = project(&ALL_SCRIPTS);
    let mut table = table_for(&root);
    let source = concat!(
        "ScriptName SelfArgLeaf Extends SelfArgMid\n\n",
        "SelfArgUtil Property Util Auto\n\n",
        "Function Test()\n",
        "    Util.TakesBase(self)\n",
        "EndFunction\n",
    );

    let diagnostics = papyrus_lints::check_argument_types(source, &mut table);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn still_flags_self_passed_for_a_descendant_parameter() {
    let root = project(&ALL_SCRIPTS);
    let mut table = table_for(&root);
    let source = concat!(
        "ScriptName SelfArgMid Extends SelfArgBase\n\n",
        "SelfArgUtil Property Util Auto\n\n",
        "Function Test()\n",
        "    Util.TakesLeaf(self)\n",
        "EndFunction\n",
    );

    let diagnostics = papyrus_lints::check_argument_types(source, &mut table);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("expects SelfArgLeaf"));
    assert!(diagnostics[0].message.contains("got SelfArgMid"));
}

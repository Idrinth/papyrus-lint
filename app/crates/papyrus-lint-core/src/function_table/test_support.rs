//! Shared helpers for this module's unit tests.

use super::FunctionTable;
use std::fs;
use std::path::Path;

pub(in crate::function_table) fn write_script(dir: &Path, name: &str, contents: &str) {
    let source_dir = dir.join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    fs::write(source_dir.join(format!("{name}.psc")), contents)
        .expect("failed to write test script file");
}

pub(in crate::function_table) fn diagnostics_for(
    rule: &str,
    source: &str,
    table: &mut FunctionTable,
) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lints::lint_with_external_arguments(source, &papyrus_lints::Config::default(), table)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == rule)
        .collect()
}

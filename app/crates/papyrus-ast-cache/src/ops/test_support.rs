//! Shared helpers for [`super::load`], [`super::store`] and [`super::prime`]'s
//! unit tests.

use crate::entry::CacheEntry;
use crate::version::MIN_COMPATIBLE_VERSION;
use tempfile::tempdir;

/// A version at the minimum compatible threshold, used by tests that don't
/// care about version compatibility itself.
pub(in crate::ops) const COMPATIBLE_VERSION: &str = MIN_COMPATIBLE_VERSION;

pub(in crate::ops) fn sample_ast() -> papyrus_parser::ast::Script {
    papyrus_parser::parse("ScriptName Example\n").unwrap()
}

pub(in crate::ops) fn sample_tokens() -> Vec<papyrus_parser::token::Token> {
    papyrus_parser::tokenize("ScriptName Example\n").unwrap()
}

pub(in crate::ops) struct Harness {
    pub(in crate::ops) cache_dir: tempfile::TempDir,
    _project_dir: tempfile::TempDir,
    pub(in crate::ops) source_path: std::path::PathBuf,
    pub(in crate::ops) source: &'static str,
}

pub(in crate::ops) fn harness(filename: &str, source: &'static str) -> Harness {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join(filename);
    std::fs::write(&source_path, source).unwrap();
    Harness {
        cache_dir,
        _project_dir: project_dir,
        source_path,
        source,
    }
}

pub(in crate::ops) fn write_raw(h: &Harness, entry: CacheEntry) {
    std::fs::create_dir_all(h.cache_dir.path()).unwrap();
    std::fs::write(
        crate::entry::cache_file_path(h.cache_dir.path(), &h.source_path),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();
}

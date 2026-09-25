use std::path::{Path, PathBuf};

use papyrus_lint_config::{load_config, load_config_from_path};
use papyrus_lints::Config;

/// Resolves lint configuration for a blob that has no on-disk script path.
///
/// Given `Some(path)`, loads that file through
/// [`load_config_from_path`] — the same contract as `--config <path>`.
/// Given `None`, returns [`Config::default`], because there is no project
/// root to discover a `papyrus-lint.yaml`/`.yml` from.
pub fn config_from_override(path: Option<&Path>) -> Result<Config, String> {
    match path {
        Some(path) => load_config_from_path(path),
        None => Ok(Config::default()),
    }
}

/// Resolves lint configuration for an open buffer that has an on-disk path.
///
/// Walks from `script_path`'s parent directory up to the filesystem root
/// and loads the first `papyrus-lint.yaml`/`.yml` found. A missing or
/// unreadable config file is treated as [`Config::default`], matching the
/// language server's previous "don't fail the document sync" behavior.
pub fn config_from_script_path(script_path: &Path) -> Config {
    let mut dir = script_path.parent();
    while let Some(current) = dir {
        if config_file(current).is_some() {
            return load_config(current).unwrap_or_else(|_| Config::default());
        }
        dir = current.parent();
    }
    Config::default()
}

fn config_file(dir: &Path) -> Option<PathBuf> {
    for name in ["papyrus-lint.yaml", "papyrus-lint.yml"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

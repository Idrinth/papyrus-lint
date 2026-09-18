//! Implementation modules for `build.rs`.

pub mod data_tables;
pub mod dispatch;
pub mod metadata;
pub mod renderer;

use serde::de::DeserializeOwned;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub struct BuildContext {
    manifest_dir: PathBuf,
    out_dir: PathBuf,
}

impl BuildContext {
    pub fn from_env() -> Self {
        Self {
            manifest_dir: env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .expect("CARGO_MANIFEST_DIR is set by cargo"),
            out_dir: env::var_os("OUT_DIR")
                .map(PathBuf::from)
                .expect("OUT_DIR is set by cargo"),
        }
    }

    pub fn input(&self, relative: &str) -> PathBuf {
        self.manifest_dir.join("../../..").join(relative)
    }

    pub fn load_yaml<T: DeserializeOwned>(&self, relative: &str, description: &str) -> T {
        self.load(relative, description, |source| {
            serde_norway::from_str(source)
        })
    }

    pub fn load_json<T: DeserializeOwned>(&self, relative: &str, description: &str) -> T {
        self.load(relative, description, |source| serde_json::from_str(source))
    }

    fn load<T, E>(
        &self,
        relative: &str,
        description: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
    ) -> T
    where
        E: std::fmt::Display,
    {
        let path = self.input(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        let source = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read {description} at {}: {error}",
                path.display()
            )
        });
        parse(&source).unwrap_or_else(|error| {
            panic!(
                "failed to parse {description} at {}: {error}",
                path.display()
            )
        })
    }

    pub fn load_text(&self, relative: &str, description: &str) -> String {
        let path = self.input(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read {description} at {}: {error}",
                path.display()
            )
        })
    }

    pub fn write(&self, filename: &str, description: &str, contents: &str) {
        let destination = self.out_dir.join(filename);
        fs::write(&destination, contents).unwrap_or_else(|error| {
            panic!(
                "failed to write generated {description} to {}: {error}",
                destination.display()
            )
        });
    }
}

pub fn generated_header(source: &str) -> String {
    format!("/// Compiled from `{source}` by `build.rs`. Do not edit by hand.")
}

pub fn default_config_order(source: &str, path: &Path) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    let mut in_rules = false;
    for line in source.lines() {
        if line == "rules:" {
            in_rules = true;
            continue;
        }
        if !in_rules {
            continue;
        }
        if !line.starts_with("  ") || line.starts_with("   ") {
            break;
        }
        let Some((key, _)) = line.trim().split_once(':') else {
            break;
        };
        keys.push(key.to_string());
    }
    if keys.is_empty() {
        return Err(format!("{} has no `rules:` entries", path.display()));
    }
    Ok(keys)
}

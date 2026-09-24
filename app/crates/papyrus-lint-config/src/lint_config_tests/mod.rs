use super::*;
use crate::config_file_path;
use papyrus_lints::config::Indentation;
use std::fs;
use std::path::Path;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

mod config;
mod discovery_and_loading;

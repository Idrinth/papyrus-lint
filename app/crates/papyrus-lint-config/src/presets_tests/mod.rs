use std::fs;

use super::*;
use crate::{config_file_path, load_config_from_path};

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

mod add;
mod initialize;
mod manage;
mod preset;
mod preset_config;
mod save;

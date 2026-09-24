use super::*;
use crate::project_file::project_file_from_yaml;
use crate::{load_config, save_config};
use std::fs;
use std::path::Path;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

mod additional;
mod lookup;

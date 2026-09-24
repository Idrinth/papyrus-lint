use super::*;
use crate::{load_config, save_config};
use std::fs;
use std::path::Path;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

mod compile_check;
mod compiler_path;

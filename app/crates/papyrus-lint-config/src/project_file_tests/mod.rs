use super::*;
use papyrus_lints::config::Indentation;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

mod compile_check;
mod compiler_path;
mod discovery_and_loading;
mod lint_config;
mod lookup_script_roots;
mod script_roots;
mod strict_achlist_scope;

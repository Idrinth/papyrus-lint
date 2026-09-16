use std::fs;
use std::path::Path;

pub(crate) fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

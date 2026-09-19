//! Compiles `shared/rules/data/native-globals.yaml` into a static Rust array
//! at build time, so `native_globals::is_known` never parses YAML at runtime
//! (see `src/native_globals.rs`).

use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct RawNativeGlobal {
    script: String,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    compile_native_globals(&manifest_dir, &out_dir);
}

fn compile_native_globals(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../shared/rules/data/native-globals.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read native-globals rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawNativeGlobal> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse native-globals rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `shared/rules/data/native-globals.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("const NATIVE_GLOBALS: &[&str] = &[\n");
    for rule in &rules {
        generated.push_str(&format!("    {:?},\n", rule.script.to_ascii_lowercase()));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("native_globals_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

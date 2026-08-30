//! Compiles `rules/native-types.yaml` into a static Rust array at build
//! time, so `native_types::parent_of` never parses YAML at runtime (see
//! `src/native_types.rs`).

use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct RawNativeType {
    #[serde(rename = "type")]
    type_name: String,
    extends: String,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    let yaml_path = Path::new(&manifest_dir).join("../../../rules/native-types.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read native-types rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawNativeType> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse native-types rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/native-types.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("const NATIVE_EXTENDS: &[(&str, &str)] = &[\n");
    for rule in &rules {
        generated.push_str(&format!(
            "    ({:?}, {:?}),\n",
            rule.type_name.to_ascii_lowercase(),
            rule.extends.to_ascii_lowercase()
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(&out_dir).join("native_types_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

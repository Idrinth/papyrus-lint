//! Compiles `rules/forbidden-functions.yaml` into a static Rust array at
//! build time, so the shipped linter never parses YAML at runtime (see
//! `src/lints/forbidden_functions.rs`).

use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct RawRule {
    script: String,
    function: String,
    level: String,
    message: String,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let yaml_path = Path::new(&manifest_dir).join("../../rules/forbidden-functions.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read forbidden-functions rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawRule> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse forbidden-functions rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/forbidden-functions.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static FORBIDDEN_FUNCTIONS: &[ForbiddenFunctionRule] = &[\n");
    for rule in &rules {
        let level = match rule.level.as_str() {
            "error" => "Level::Error",
            "warning" => "Level::Warning",
            "info" => "Level::Info",
            other => panic!(
                "forbidden-functions.yaml: unknown level `{other}` for {}.{}",
                rule.script, rule.function
            ),
        };
        generated.push_str(&format!(
            "    ForbiddenFunctionRule {{ script: {:?}, function: {:?}, level: {level}, message: {:?} }},\n",
            rule.script, rule.function, rule.message
        ));
    }
    generated.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let dest = Path::new(&out_dir).join("forbidden_functions_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

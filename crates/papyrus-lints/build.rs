//! Compiles `rules/forbidden-functions.yaml` and `rules/slow-functions.yaml`
//! into static Rust arrays at build time, so `forbidden_functions::check`
//! and `slow_functions::check` never parse YAML at runtime (see
//! `src/forbidden_functions.rs` and `src/slow_functions.rs`).

use std::env;
use std::fs;
use std::path::Path;

#[derive(serde::Deserialize)]
struct RawForbiddenRule {
    script: String,
    function: String,
    level: String,
    message: String,
    /// Whether `script` is a native singleton (e.g. `Game`, `Utility`) that
    /// is always called through its literal script name rather than
    /// through a variable of some subclass. When true, a qualified call
    /// only matches this rule if its qualifier is literally `script`
    /// (case-insensitively) — see `forbidden_functions::check`.
    #[serde(default)]
    global: bool,
}

#[derive(serde::Deserialize)]
struct RawSlowRule {
    object: String,
    function: String,
    replacement: String,
    /// See `RawForbiddenRule::global`.
    #[serde(default)]
    global: bool,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    compile_forbidden_functions(&manifest_dir, &out_dir);
    compile_slow_functions(&manifest_dir, &out_dir);
}

fn compile_forbidden_functions(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../rules/forbidden-functions.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read forbidden-functions rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawForbiddenRule> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
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
        match rule.level.as_str() {
            "error" | "warning" | "info" => {}
            other => panic!(
                "forbidden-functions.yaml: unknown level `{other}` for {}.{}",
                rule.script, rule.function
            ),
        }
        generated.push_str(&format!(
            "    ForbiddenFunctionRule {{ script: {:?}, function: {:?}, level: {:?}, message: {:?}, global: {:?} }},\n",
            rule.script, rule.function, rule.level, rule.message, rule.global
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("forbidden_functions_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

fn compile_slow_functions(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../rules/slow-functions.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read slow-functions rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawSlowRule> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse slow-functions rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/slow-functions.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static SLOW_FUNCTIONS: &[SlowFunctionRule] = &[\n");
    for rule in &rules {
        generated.push_str(&format!(
            "    SlowFunctionRule {{ object: {:?}, function: {:?}, replacement: {:?}, global: {:?} }},\n",
            rule.object, rule.function, rule.replacement, rule.global
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("slow_functions_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

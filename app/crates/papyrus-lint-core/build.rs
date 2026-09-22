//! Compiles repository data into static Rust arrays used by this crate.

use std::env;
use std::fs;
use std::path::Path;

#[path = "../papyrus-lints/build_support/script_catalog.rs"]
mod script_catalog;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    compile_native_globals(&manifest_dir, &out_dir);
    compile_preset_descriptions(&manifest_dir, &out_dir);
}

fn compile_preset_descriptions(manifest_dir: &str, out_dir: &str) {
    let presets_dir = Path::new(manifest_dir).join("../../../configuration/presets");
    println!("cargo:rerun-if-changed={}", presets_dir.display());

    let mut paths: Vec<_> = fs::read_dir(&presets_dir)
        .unwrap_or_else(|err| {
            panic!(
                "failed to read preset directory at {}: {err}",
                presets_dir.display()
            )
        })
        .map(|entry| entry.expect("failed to read preset directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("papyrus-lint.") && name.ends_with(".yaml"))
        })
        .collect();
    paths.sort();

    let mut generated = String::from(
        "/// Compiled from `configuration/presets/papyrus-lint.*.yaml` header comments by `build.rs`. Do not edit by hand.\n\
         const DESCRIPTIONS: &[(&str, &str, &str)] = &[\n",
    );
    for path in paths {
        println!("cargo:rerun-if-changed={}", path.display());
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read preset at {}: {err}", path.display()));
        let id = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("papyrus-lint."))
            .and_then(|name| name.strip_suffix(".yaml"))
            .expect("filtered preset file should have a preset name");
        let label = title_case(id);
        let description = header_description(&source).unwrap_or_else(|| {
            panic!(
                "preset at {} needs a descriptive comment paragraph immediately after its banner",
                path.display()
            )
        });
        generated.push_str(&format!(
            "    ({:?}, {:?}, {:?}),\n",
            id, label, description
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("preset_descriptions.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated preset descriptions to {}: {err}",
            dest.display()
        )
    });
}

fn header_description(source: &str) -> Option<String> {
    let mut lines = source.lines();
    let banner = lines.next()?;
    if !banner.starts_with("# === ") {
        return None;
    }

    let description: Vec<_> = lines
        .take_while(|line| line.starts_with('#') && *line != "#")
        .map(|line| {
            line.strip_prefix("# ")
                .unwrap_or(line.trim_start_matches('#'))
        })
        .collect();
    (!description.is_empty()).then(|| description.join(" "))
}

fn title_case(id: &str) -> String {
    let mut chars = id.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn compile_native_globals(manifest_dir: &str, out_dir: &str) {
    let scripts_dir = Path::new(manifest_dir).join("../../../shared/scripts");

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from bundled Creation Kit / script-extender archives by `build.rs`. Do not edit by hand.\n",
    );
    for game in script_catalog::GAMES {
        let rules = script_catalog::native_global_names(&scripts_dir, game);
        let const_name = match *game {
            "fallout4" => "FALLOUT4_NATIVE_GLOBALS",
            _ => "SKYRIM_NATIVE_GLOBALS",
        };
        generated.push_str(&format!("const {const_name}: &[&str] = &[\n"));
        for script in &rules {
            generated.push_str(&format!("    {:?},\n", script));
        }
        generated.push_str("];\n");
    }
    generated.push_str("const NATIVE_GLOBALS: &[&str] = SKYRIM_NATIVE_GLOBALS;\n");

    let dest = Path::new(out_dir).join("native_globals_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

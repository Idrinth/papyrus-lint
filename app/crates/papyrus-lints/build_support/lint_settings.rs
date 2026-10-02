//! Generates `Config` (every field except [`Rules`]) from
//! `shared/configuration/lint-settings/*.json`, in the order listed by
//! `shared/configuration/lint-settings.yaml`.

use super::renderer::Renderer;
use super::BuildContext;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;

#[derive(Debug, Deserialize)]
struct SettingsIndex {
    settings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LintSetting {
    key: String,
    rust: RustMeta,
    doc: String,
}

#[derive(Debug, Deserialize)]
struct RustMeta {
    #[serde(rename = "type")]
    type_name: String,
    default: String,
}

pub fn compile(context: &BuildContext) {
    let index: SettingsIndex = context.load_yaml(
        "shared/configuration/lint-settings.yaml",
        "lint settings index",
    );
    let dir = context.input("shared/configuration/lint-settings");
    println!("cargo:rerun-if-changed={}", dir.display());
    let found = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    let listed: BTreeSet<_> = index.settings.iter().cloned().collect();
    let extras: Vec<_> = found.difference(&listed).cloned().collect();
    if !extras.is_empty() {
        panic!(
            "{} has setting files not listed in shared/configuration/lint-settings.yaml `settings`: {}",
            dir.display(),
            extras.join(", ")
        );
    }
    let missing: Vec<_> = listed.difference(&found).cloned().collect();
    if !missing.is_empty() {
        panic!(
            "shared/configuration/lint-settings.yaml `settings` lists keys with no JSON file: {}",
            missing.join(", ")
        );
    }
    if index.settings.len() != listed.len() {
        panic!("shared/configuration/lint-settings.yaml `settings` lists a key more than once");
    }
    let mut settings = Vec::with_capacity(index.settings.len());
    for key in &index.settings {
        let relative = format!("shared/configuration/lint-settings/{key}.json");
        let setting: LintSetting = context.load_json(&relative, "lint setting");
        if setting.key != *key {
            panic!(
                "{relative}: `key` is {:?}, expected {key:?} to match the file name",
                setting.key
            );
        }
        settings.push(setting);
    }
    validate(&settings);
    context.write("config_struct.rs", "Config struct", &render(&settings));
}

fn validate(settings: &[LintSetting]) {
    if settings.is_empty() {
        panic!("shared/configuration/lint-settings has no settings");
    }
    let mut seen = BTreeSet::new();
    for setting in settings {
        if !seen.insert(setting.key.clone()) {
            panic!(
                "shared/configuration/lint-settings lists `{}` more than once",
                setting.key
            );
        }
        if setting.rust.default.contains(['\n', ';', '{']) {
            panic!(
                "shared/configuration/lint-settings/{}.json: rust.default must be a single expression",
                setting.key
            );
        }
    }
}

fn render(settings: &[LintSetting]) -> String {
    let mut out = Renderer::new();
    out.line("/// Configuration for the lint/fix jobs, deserialized from a project's YAML");
    out.line("/// config file and, in the desktop app, kept in sync with the formatting");
    out.line("/// controls in the UI (loaded on startup, saved back to the file whenever");
    out.line("/// they change). Fields absent from the YAML fall back to their default.");
    out.line("/// File I/O for that YAML lives in `papyrus-lint-config`.");
    out.line("///");
    out.line("/// Generated from `shared/configuration/lint-settings/*.json` by `build.rs`.");
    out.line("/// Do not edit by hand. `rules` is the exception: that struct is");
    out.line("/// generated from `shared/rules/*.json`.");
    out.line("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]");
    out.line("#[serde(default)]");
    out.block("pub struct Config", |out| {
        for setting in settings {
            for line in setting.doc.lines() {
                if line.is_empty() {
                    out.line("///");
                } else {
                    out.line(format_args!("/// {line}"));
                }
            }
            out.line(format_args!(
                "pub {}: {},",
                setting.key, setting.rust.type_name
            ));
        }
        out.line("/// Per-ruleset enable/disable switches. Every ruleset is enabled by");
        out.line("/// default unless `shared/rules/<id>.json` sets `enabled_by_default: false`;");
        out.line("/// see [`Rules`].");
        out.line("pub rules: Rules,");
    });
    out.blank();
    out.line("impl Default for Config {");
    out.line("    fn default() -> Self {");
    out.line("        Self {");
    for setting in settings {
        out.line(format_args!(
            "            {}: {},",
            setting.key, setting.rust.default
        ));
    }
    out.line("            rules: Rules::default(),");
    out.line("        }");
    out.line("    }");
    out.line("}");
    out.finish()
}

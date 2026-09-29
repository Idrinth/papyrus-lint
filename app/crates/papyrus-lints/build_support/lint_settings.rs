//! Generates `Config` (every field except [`Rules`]) from
//! `shared/configuration/lint-settings.yaml`.

use super::renderer::Renderer;
use super::BuildContext;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const RUST_TYPES: &[&str] = &[
    "bool",
    "usize",
    "f64",
    "Game",
    "Indentation",
    "IdentifierCasing",
    "TypeCasing",
    "NamedArguments",
    "MagicNumbers",
    "Hungarian",
    "EncodingEnforced",
    "LineEndingsMode",
];

#[derive(Debug, Deserialize)]
struct LintSettingsFile {
    settings: Vec<LintSetting>,
}

#[derive(Debug, Deserialize)]
struct LintSetting {
    key: String,
    rust: RustMeta,
    yaml: YamlMeta,
    doc: String,
}

#[derive(Debug, Deserialize)]
struct RustMeta {
    #[serde(rename = "type")]
    type_name: String,
    default: String,
}

#[derive(Debug, Deserialize)]
struct YamlMeta {
    default: String,
}

pub fn compile(context: &BuildContext) {
    let file: LintSettingsFile =
        context.load_yaml("shared/configuration/lint-settings.yaml", "lint settings");
    let relative = "shared/configuration/papyrus-lint.default.yaml";
    let source = context.load_text(relative, "default config");
    let top = default_config_top(&source);
    validate(&file.settings, &top);
    context.write("config_struct.rs", "Config struct", &render(&file.settings));
}

fn validate(settings: &[LintSetting], top: &BTreeMap<String, String>) {
    if settings.is_empty() {
        panic!("shared/configuration/lint-settings.yaml has no settings");
    }
    let mut seen = BTreeSet::new();
    for setting in settings {
        if !seen.insert(setting.key.clone()) {
            panic!(
                "shared/configuration/lint-settings.yaml lists `{}` more than once",
                setting.key
            );
        }
        if !RUST_TYPES.contains(&setting.rust.type_name.as_str()) {
            panic!(
                "shared/configuration/lint-settings.yaml: `{}` has unknown rust.type `{}`",
                setting.key, setting.rust.type_name
            );
        }
        if setting.rust.default.contains(['\n', ';', '{']) {
            panic!(
                "shared/configuration/lint-settings.yaml: `{}` rust.default must be a single expression",
                setting.key
            );
        }
        match top.get(&setting.key) {
            Some(value) if value == &setting.yaml.default => {}
            Some(value) => panic!(
                "shared/configuration/papyrus-lint.default.yaml sets {} to {value}, but shared/configuration/lint-settings.yaml says {}",
                setting.key, setting.yaml.default
            ),
            None => panic!(
                "shared/configuration/papyrus-lint.default.yaml is missing {}; add it next to the other top-level keys",
                setting.key
            ),
        }
    }
}

fn default_config_top(source: &str) -> BTreeMap<String, String> {
    let mut top = BTreeMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "rules:" {
            break;
        }
        if line.starts_with(' ') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            panic!("default YAML line is not key: value: {line:?}");
        };
        top.insert(key.trim().to_string(), value.trim().to_string());
    }
    top
}

fn render(settings: &[LintSetting]) -> String {
    let mut out = Renderer::new();
    out.line("/// Configuration for the lint/fix jobs, deserialized from a project's YAML");
    out.line("/// config file and, in the desktop app, kept in sync with the formatting");
    out.line("/// controls in the UI (loaded on startup, saved back to the file whenever");
    out.line("/// they change). Fields absent from the YAML fall back to their default.");
    out.line("/// File I/O for that YAML lives in `papyrus-lint-config`.");
    out.line("///");
    out.line("/// Generated from `shared/configuration/lint-settings.yaml` by `build.rs`.");
    out.line("/// Do not edit by hand. `rules` is the exception: that struct is");
    out.line("/// generated from `shared/rules.json`.");
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
        out.line("/// default unless `shared/rules.json` sets `enabled_by_default: false`;");
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

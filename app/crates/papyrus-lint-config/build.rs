//! Generates the saved-config comment injector and each built-in preset's
//! full, annotated YAML at build time from the per-setting JSON files and
//! `shared/rules/*.json`. This avoids separately maintaining the comments in
//! Rust, avoids checking in three near-complete preset copies, and does not
//! require a generated `papyrus-lint.default.yaml` on disk before `cargo build`.
//! For each preset this layers two things onto the rendered default config:
//!
//! - the small `shared/configuration/presets/papyrus-lint.<name>.yaml` overwrite file (a
//!   header comment plus any non-rule settings the preset changes, e.g.
//!   `careful`'s relaxed cyclomatic complexity thresholds);
//! - for `standard`/`careful`, every `rules:` toggle that's `true` by
//!   default and tagged `"low"` importance in `shared/rules/<id>.json` is turned
//!   off, except the handful marked `kept_in_standard`
//!   (the cheap, auto-fixable formatting rules `standard` keeps on) — see
//!   [`preset_rule_value`].
//!
//! The merged output for each preset is written to
//! `$OUT_DIR/papyrus-lint.<name>.yaml`, which `presets.rs` embeds via
//! `include_str!` exactly as it used to embed the checked-in file directly.
//! The rendered default itself is written to `$OUT_DIR/papyrus-lint.default.yaml`
//! so tests can compare `init` output against it.

use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

/// Built-in preset names, matching `papyrus_lint_config::presets::PRESET_NAMES`
/// (duplicated here since a build script can't depend on its own crate).
const PRESET_NAMES: [&str; 3] = ["strict", "standard", "careful"];

/// The subset of a `shared/rules/<id>.json` entry this build script needs.
/// Mirrors `papyrus-lints/build.rs`'s own rule metadata, but only the fields
/// used to render the default YAML and to decide a rule's value under
/// `standard`/`careful` (see [`preset_rule_value`]).
#[derive(serde::Deserialize)]
struct RuleEntry {
    id: String,
    description: String,
    importance: String,
    #[serde(default = "enabled_by_default")]
    enabled_by_default: bool,
    #[serde(default)]
    kept_in_standard: bool,
}

fn enabled_by_default() -> bool {
    true
}

/// `shared/rules/<id>.json` `id`s (hyphenated) that don't turn into their
/// `Config.rules` toggle name (see `papyrus-lints/src/config.rs`) by simply
/// replacing `-` with `_` — everything else does.
const RULE_ID_TO_CONFIG_KEY: [(&str, &str); 2] = [
    ("float-to-int", "float_int_conversion"),
    ("too-many-named-states", "too_many_states"),
];

/// `id`'s `Config.rules` toggle name (as it appears in the rendered default
/// YAML's `rules:` section) — the key [`preset_rule_value`] looks up in `rule_meta`.
fn config_key_for(id: &str) -> String {
    RULE_ID_TO_CONFIG_KEY
        .iter()
        .find(|(rule_id, _)| *rule_id == id)
        .map(|(_, config_key)| config_key.to_string())
        .unwrap_or_else(|| id.replace('-', "_"))
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.join("../../..");
    let shared_dir = repo_root.join("shared");
    let configuration_dir = shared_dir.join("configuration");

    let meta_path = configuration_dir.join("lint-settings.yaml");
    let meta = load_settings_meta(&meta_path);
    let project = load_setting_group(
        &configuration_dir.join("project-settings"),
        &meta.project,
        "project",
    );
    let settings = load_setting_group(
        &configuration_dir.join("lint-settings"),
        &meta.settings,
        "settings",
    );
    let rules = load_rules(&shared_dir.join("rules"));
    let default_yaml = render_default_yaml(&project, &settings, &meta.rules_comment, &rules);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("papyrus-lint.default.yaml"), &default_yaml)
        .unwrap_or_else(|err| panic!("failed to write rendered default config: {err}"));
    fs::write(
        out_dir.join("comments.rs"),
        generate_comments(&default_yaml),
    )
    .unwrap_or_else(|err| panic!("failed to write generated comments module: {err}"));

    let rule_meta: HashMap<String, (String, bool)> = rules
        .iter()
        .map(|rule| {
            (
                config_key_for(&rule.id),
                (rule.importance.clone(), rule.kept_in_standard),
            )
        })
        .collect();

    for name in PRESET_NAMES {
        let overwrite_path = configuration_dir.join(format!("presets/papyrus-lint.{name}.yaml"));
        println!("cargo:rerun-if-changed={}", overwrite_path.display());
        let overwrite_yaml = fs::read_to_string(&overwrite_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", overwrite_path.display()));

        let merged = merge_preset(&default_yaml, &overwrite_yaml, name, &rule_meta);
        fs::write(out_dir.join(format!("papyrus-lint.{name}.yaml")), merged)
            .unwrap_or_else(|err| panic!("failed to write generated {name} preset: {err}"));
    }
}

#[derive(serde::Deserialize)]
struct SettingsMeta {
    rules_comment: String,
    project: Vec<String>,
    settings: Vec<String>,
}

#[derive(serde::Deserialize)]
struct SettingFile {
    key: String,
    yaml: SettingYaml,
}

#[derive(serde::Deserialize)]
struct SettingYaml {
    default: String,
    comment: String,
}

fn watch(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn read_to_string(path: &Path, description: &str) -> String {
    watch(path);
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {description} at {}: {err}", path.display()))
}

fn load_settings_meta(path: &Path) -> SettingsMeta {
    let source = read_to_string(path, "lint settings index");
    serde_norway::from_str(&source)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn load_setting_group(dir: &Path, keys: &[String], field: &str) -> Vec<SettingFile> {
    watch(dir);
    let found: BTreeSet<String> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    let listed: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    let extras: Vec<_> = found
        .iter()
        .filter(|name| !listed.contains(name.as_str()))
        .cloned()
        .collect();
    if !extras.is_empty() {
        panic!(
            "{} has setting files not listed in lint-settings.yaml `{field}`: {}",
            dir.display(),
            extras.join(", ")
        );
    }
    if keys.len() != listed.len() {
        panic!("lint-settings.yaml `{field}` lists a key more than once");
    }
    keys.iter()
        .map(|key| {
            if !found.contains(key) {
                panic!(
                    "lint-settings.yaml `{field}` lists {key:?} but {}/{key}.json does not exist",
                    dir.display()
                );
            }
            let path = dir.join(format!("{key}.json"));
            let source = read_to_string(&path, "setting");
            let setting: SettingFile = serde_json::from_str(&source)
                .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
            if setting.key != *key {
                panic!(
                    "{}: `key` is {:?}, expected {key:?} to match the file name",
                    path.display(),
                    setting.key
                );
            }
            setting
        })
        .collect()
}

fn load_rules(dir: &Path) -> Vec<RuleEntry> {
    watch(dir);
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    paths.sort();
    if paths.is_empty() {
        panic!("no rule files found in {}", dir.display());
    }
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .map(|path| {
            let source = read_to_string(&path, "rule metadata");
            let rule: RuleEntry = serde_json::from_str(&source)
                .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("");
            if rule.id != stem {
                panic!(
                    "{}: `id` is {:?}, expected {stem:?} to match the file name",
                    path.display(),
                    rule.id
                );
            }
            let key = config_key_for(&rule.id);
            if !seen.insert(key.clone()) {
                panic!("duplicate rule config key: {key}");
            }
            rule
        })
        .collect()
}

fn comment_block(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            if line.is_empty() {
                "#".to_string()
            } else {
                format!("# {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Same document `.github/scripts/ci_lib/default_config.py` renders for Pages
/// and the release archive: game, then project settings, then the other lint
/// settings, then rules in config-key order.
fn render_default_yaml(
    project: &[SettingFile],
    settings: &[SettingFile],
    rules_comment: &str,
    rules: &[RuleEntry],
) -> String {
    let mut by_key: Vec<(String, &RuleEntry)> = rules
        .iter()
        .map(|rule| (config_key_for(&rule.id), rule))
        .collect();
    by_key.sort_by(|left, right| left.0.cmp(&right.0));

    let mut lines = Vec::new();
    let game = settings
        .iter()
        .find(|setting| setting.key == "game")
        .unwrap_or_else(|| panic!("shared/configuration/lint-settings is missing game"));
    lines.push(comment_block(&game.yaml.comment));
    lines.push(format!("game: {}", game.yaml.default));
    for setting in project {
        lines.push(comment_block(&setting.yaml.comment));
        lines.push(format!("{}: {}", setting.key, setting.yaml.default));
    }
    for setting in settings {
        if setting.key == "game" {
            continue;
        }
        lines.push(comment_block(&setting.yaml.comment));
        lines.push(format!("{}: {}", setting.key, setting.yaml.default));
    }
    lines.push(comment_block(rules_comment));
    lines.push("rules:".to_string());
    for (key, rule) in by_key {
        let enabled = if rule.enabled_by_default {
            "true"
        } else {
            "false"
        };
        lines.push(format!("  {key}: {enabled} # {}", rule.description));
    }
    lines.push(String::new());
    lines.join("\n")
}

/// Generates the comment-injection module from the annotated default config,
/// making that YAML file the single source of truth for saved-file comments.
fn generate_comments(default: &str) -> String {
    let mut fields = Vec::new();
    let mut comments = Vec::new();
    let mut rule_comments = Vec::new();

    for line in default.lines() {
        if line.starts_with("# ") {
            comments.push(line);
        } else if !line.starts_with(' ') && !line.trim().is_empty() {
            let key = line
                .split(':')
                .next()
                .unwrap_or_else(|| panic!("top-level config line {line:?} has no key"));
            assert!(
                !comments.is_empty(),
                "top-level config key {key:?} has no explanatory comment"
            );
            fields.push((key, comments.join("\n")));
            comments.clear();
        } else if let Some((setting, comment)) = line.trim().split_once(" #") {
            let key = setting
                .split(':')
                .next()
                .unwrap_or_else(|| panic!("rule config line {line:?} has no key"));
            rule_comments.push((key, comment.trim_start()));
        }
    }

    let mut generated = String::from(
        "// @generated by build.rs from the rendered default configuration.\n\
         // Do not edit this file directly.\n\n\
         const FIELD_COMMENTS: &[(&str, &str)] = &[\n",
    );
    for (key, comment) in fields {
        writeln!(generated, "    ({key:?}, {comment:?}),").expect("write to String");
    }
    generated.push_str(
        "];

const RULE_COMMENTS: &[(&str, &str)] = &[
",
    );
    for (key, comment) in rule_comments {
        writeln!(generated, "    ({key:?}, {comment:?}),").expect("write to String");
    }
    generated.push_str(
        "];\n\n\
         /// Restores the default config's comments on serialized settings and rules.\n\
         pub(crate) fn with_field_comments(yaml: &str) -> String {\n\
         \x20   let mut out = String::with_capacity(yaml.len() + (FIELD_COMMENTS.len() + RULE_COMMENTS.len()) * 32);\n\
         \x20   let mut in_rules = false;\n\
         \x20   for line in yaml.lines() {\n\
         \x20       if !line.starts_with(' ') {\n\
         \x20           if let Some(key) = line.split(':').next() {\n\
         \x20               in_rules = key == \"rules\";\n\
         \x20               if let Some((_, comment)) = FIELD_COMMENTS.iter().find(|(name, _)| *name == key) {\n\
         \x20                   out.push_str(comment);\n\
         \x20                   out.push('\\n');\n\
         \x20               }\n\
         \x20           }\n\
         \x20       }\n\
         \x20       out.push_str(line);\n\
         \x20       if in_rules && line.starts_with(\"  \") {\n\
         \x20           if let Some(key) = line.trim().split(':').next() {\n\
         \x20               if let Some((_, comment)) = RULE_COMMENTS.iter().find(|(name, _)| *name == key) {\n\
         \x20                   out.push_str(\" # \");\n\
         \x20                   out.push_str(comment);\n\
         \x20               }\n\
         \x20           }\n\
         \x20       }\n\
         \x20       out.push('\\n');\n\
         \x20   }\n\
         \x20   out\n\
         }\n",
    );
    generated
}

/// Applies `overwrite`'s header and non-rule overrides, plus `preset`'s
/// [`preset_rule_value`]-derived `rules:` toggles, onto `default`,
/// reproducing the full annotated YAML a preset used to be checked in as
/// verbatim.
///
/// `overwrite` is: a leading block of comment/blank lines (copied verbatim
/// as the header), followed by zero or more top-level `key: value` override
/// lines (there is currently no rule that needs a hand-written `rules:`
/// override here, since [`preset_rule_value`] derives every rule's value
/// from `shared/rules/<id>.json`). Each override line replaces the matching key's
/// line in `default` outright; every other top-level line of `default`
/// (including its own per-field comments) is kept as-is.
fn merge_preset(
    default: &str,
    overwrite: &str,
    preset: &str,
    rule_meta: &HashMap<String, (String, bool)>,
) -> String {
    let overwrite_lines: Vec<&str> = overwrite.lines().collect();
    let header_len = overwrite_lines
        .iter()
        .take_while(|line| line.trim().is_empty() || line.trim_start().starts_with('#'))
        .count();
    let (header, body) = overwrite_lines.split_at(header_len);

    let mut top_overrides: HashMap<&str, &str> = HashMap::new();
    for line in body {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.split(':').next().expect("key:value line").trim();
        top_overrides.insert(key, line);
    }

    let mut in_rules = false;
    let mut merged_lines: Vec<String> = Vec::with_capacity(default.lines().count());
    for line in default.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            merged_lines.push(line.to_string());
            continue;
        }
        if !line.starts_with(' ') {
            let key = trimmed.split(':').next().expect("key:value line").trim();
            in_rules = key == "rules";
            merged_lines.push(top_overrides.get(key).copied().unwrap_or(line).to_string());
        } else if in_rules {
            let mut parts = trimmed.splitn(2, ':');
            let rule_id = parts.next().expect("key:value line").trim();
            let value_and_comment = parts
                .next()
                .unwrap_or_else(|| panic!("rule line {trimmed:?} has no value"))
                .trim();
            let (default_text, comment) = value_and_comment
                .split_once(" #")
                .map_or((value_and_comment, ""), |(value, comment)| {
                    (value, comment.trim_start())
                });
            let default_value = default_text == "true";
            let value = preset_rule_value(preset, rule_id, default_value, rule_meta);
            if comment.is_empty() {
                merged_lines.push(format!("  {rule_id}: {value}"));
            } else {
                merged_lines.push(format!("  {rule_id}: {value} # {comment}"));
            }
        } else {
            merged_lines.push(line.to_string());
        }
    }

    let mut result = header.join("\n");
    if !header.is_empty() {
        result.push('\n');
    }
    result.push_str(&merged_lines.join("\n"));
    result.push('\n');
    result
}

/// `rule_id`'s boolean value under `preset`, derived from `default_value`
/// (its value in the rendered default YAML, i.e. under `strict`) and
/// its `importance`/`kept_in_standard` metadata in `shared/rules/<id>.json`:
/// `careful` turns off every `"low"` importance rule; `standard` does the
/// same except for the handful marked `kept_in_standard` (the cheap,
/// auto-fixable formatting rules). A rule already `false` by default is
/// never turned back on, and `strict` never overrides a rule at all.
fn preset_rule_value(
    preset: &str,
    rule_id: &str,
    default_value: bool,
    rule_meta: &HashMap<String, (String, bool)>,
) -> bool {
    if !default_value || preset == "strict" {
        return default_value;
    }
    let (importance, kept_in_standard) = rule_meta
        .get(rule_id)
        .unwrap_or_else(|| panic!("no shared/rules entry for rule {rule_id:?}"));
    if importance != "low" {
        return true;
    }
    match preset {
        "careful" => false,
        "standard" => *kept_in_standard,
        _ => true,
    }
}

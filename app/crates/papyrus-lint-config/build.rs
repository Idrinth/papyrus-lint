//! Generates the saved-config comment injector and each built-in preset's
//! full, annotated YAML at build time from
//! `configuration/papyrus-lint.default.yaml`. This avoids separately maintaining
//! the comments in Rust and avoids checking in three near-complete preset copies.
//! For each preset this layers two things onto the default config:
//!
//! - the small `configuration/presets/papyrus-lint.<name>.yaml` overwrite file (a
//!   header comment plus any non-rule settings the preset changes, e.g.
//!   `careful`'s relaxed cyclomatic complexity thresholds);
//! - for `standard`/`careful`, every `rules:` toggle that's `true` by
//!   default and tagged `"low"` importance in `shared/rules.json` is turned
//!   off, except the handful `shared/rules.json` marks `kept_in_standard`
//!   (the cheap, auto-fixable formatting rules `standard` keeps on) — see
//!   [`preset_rule_value`].
//!
//! The merged output for each preset is written to
//! `$OUT_DIR/papyrus-lint.<name>.yaml`, which `presets.rs` embeds via
//! `include_str!` exactly as it used to embed the checked-in file directly.

use std::collections::HashMap;
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

/// Built-in preset names, matching `papyrus_lint_config::presets::PRESET_NAMES`
/// (duplicated here since a build script can't depend on its own crate).
const PRESET_NAMES: [&str; 3] = ["strict", "standard", "careful"];

/// The subset of a `shared/rules.json` entry this build script needs to
/// decide a rule's value under `standard`/`careful` (see
/// [`preset_rule_value`]). Mirrors `papyrus-lints/build.rs`'s own
/// `RawRuleTag`, but only the fields used here.
#[derive(serde::Deserialize)]
struct RuleEntry {
    id: String,
    importance: String,
    #[serde(default)]
    kept_in_standard: bool,
}

/// `shared/rules.json` `id`s (hyphenated) that don't turn into their
/// `Config.rules` toggle name (see `papyrus-lints/src/config.rs`) by simply
/// replacing `-` with `_` — everything else does.
const RULE_ID_TO_CONFIG_KEY: [(&str, &str); 2] = [
    ("float-to-int", "float_int_conversion"),
    ("too-many-named-states", "too_many_states"),
];

/// `id`'s `Config.rules` toggle name (as it appears in
/// `configuration/papyrus-lint.default.yaml`'s `rules:` section) — the key
/// [`preset_rule_value`] looks up in `rule_meta`.
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
    let configuration_dir = repo_root.join("configuration");
    let shared_dir = repo_root.join("shared");

    let default_path = configuration_dir.join("papyrus-lint.default.yaml");
    println!("cargo:rerun-if-changed={}", default_path.display());
    let default_yaml = fs::read_to_string(&default_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", default_path.display()));

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(
        out_dir.join("comments.rs"),
        generate_comments(&default_yaml),
    )
    .unwrap_or_else(|err| panic!("failed to write generated comments module: {err}"));

    let rules_path = shared_dir.join("rules.json");
    println!("cargo:rerun-if-changed={}", rules_path.display());
    let rules_json = fs::read_to_string(&rules_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", rules_path.display()));
    let rule_entries: Vec<RuleEntry> = serde_json::from_str(&rules_json)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", rules_path.display()));
    let rule_meta: HashMap<String, (String, bool)> = rule_entries
        .into_iter()
        .map(|rule| {
            let config_key = config_key_for(&rule.id);
            (config_key, (rule.importance, rule.kept_in_standard))
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

/// Generates the comment-injection module from the annotated default config,
/// making that YAML file the single source of truth for saved-file comments.
fn generate_comments(default: &str) -> String {
    let mut fields = Vec::new();
    let mut comments = Vec::new();

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
        }
    }

    let mut generated = String::from(
        "// @generated by build.rs from configuration/papyrus-lint.default.yaml.\n\
         // Do not edit this file directly.\n\n\
         const FIELD_COMMENTS: &[(&str, &str)] = &[\n",
    );
    for (key, comment) in fields {
        writeln!(generated, "    ({key:?}, {comment:?}),").expect("write to String");
    }
    generated.push_str(
        "];\n\n\
         /// Inserts the default config's comments above matching top-level keys.\n\
         /// Nested keys, including individual rule toggles, are left alone.\n\
         pub(crate) fn with_field_comments(yaml: &str) -> String {\n\
         \x20   let mut out = String::with_capacity(yaml.len() + FIELD_COMMENTS.len() * 32);\n\
         \x20   for line in yaml.lines() {\n\
         \x20       if !line.starts_with(' ') {\n\
         \x20           if let Some(key) = line.split(':').next() {\n\
         \x20               if let Some((_, comment)) = FIELD_COMMENTS.iter().find(|(name, _)| *name == key) {\n\
         \x20                   out.push_str(comment);\n\
         \x20                   out.push('\\n');\n\
         \x20               }\n\
         \x20           }\n\
         \x20       }\n\
         \x20       out.push_str(line);\n\
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
/// from `shared/rules.json`). Each override line replaces the matching key's
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
            let default_value = parts
                .next()
                .unwrap_or_else(|| panic!("rule line {trimmed:?} has no value"))
                .trim()
                == "true";
            let value = preset_rule_value(preset, rule_id, default_value, rule_meta);
            merged_lines.push(format!("  {rule_id}: {value}"));
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
/// (its value in `configuration/papyrus-lint.default.yaml`, i.e. under `strict`) and
/// its `importance`/`kept_in_standard` metadata in `shared/rules.json`:
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
        .unwrap_or_else(|| panic!("no shared/rules.json entry for rule {rule_id:?}"));
    if importance != "low" {
        return true;
    }
    match preset {
        "careful" => false,
        "standard" => *kept_in_standard,
        _ => true,
    }
}

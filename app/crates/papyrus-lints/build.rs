//! Compiles `rules/forbidden-functions.yaml`, `rules/slow-functions.yaml`,
//! `rules/native-methods.yaml`, `rules/actor-values.yaml`,
//! `rules/update-event-handlers.yaml`, `rules/known-events.yaml`, and
//! `docs/rules.json` into static Rust arrays at build time, so
//! `forbidden_functions::check`, `slow_functions::check`,
//! `native_function_usage::check`, `actor_value::check`,
//! `missing_update_handler::check`, `event_signature::check`, and
//! `tags::RULE_TAGS` never parse their rule data at runtime (see
//! `src/forbidden_functions.rs`, `src/slow_functions.rs`,
//! `src/native_function_usage.rs`, `src/actor_value.rs`,
//! `src/missing_update_handler.rs`, `src/event_signature.rs`, and
//! `src/tags.rs`).
//!
//! `docs/rules.json` is also compiled into `KNOWN_RULE_IDS`/
//! `FIXABLE_RULE_IDS` (`src/registry.rs`), in the JSON array's own order,
//! so those stay in the same order as `RULE_TAGS` (see `tags.rs`'s
//! `published_rule_tags_have_the_same_order_and_cardinality_as_known_rules`
//! test).

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

#[derive(serde::Deserialize)]
struct RawNativeMethod {
    object: String,
    function: String,
}

#[derive(serde::Deserialize)]
struct RawUpdateEventPair {
    register: String,
    event: String,
}

#[derive(serde::Deserialize)]
struct RawEventArg {
    #[serde(rename = "type")]
    type_name: String,
    name: String,
}

#[derive(serde::Deserialize)]
struct RawKnownEvent {
    event: String,
    form: String,
    args: Vec<RawEventArg>,
}

#[derive(serde::Deserialize)]
struct RawRuleTag {
    id: String,
    tags: Vec<String>,
    importance: String,
    /// The rule's detailed description (see `RuleTags::description`).
    /// `docs/rules.json` also carries a shorter `description` field (the
    /// `docs/nexuspage.bbcode` blurb), which this doesn't need.
    definition: String,
}

#[derive(serde::Deserialize)]
struct RawRuleId {
    id: String,
    fixable: bool,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    compile_forbidden_functions(&manifest_dir, &out_dir);
    compile_slow_functions(&manifest_dir, &out_dir);
    compile_native_methods(&manifest_dir, &out_dir);
    compile_actor_values(&manifest_dir, &out_dir);
    compile_update_event_pairs(&manifest_dir, &out_dir);
    compile_known_events(&manifest_dir, &out_dir);
    compile_rule_tags(&manifest_dir, &out_dir);
    compile_known_rule_ids(&manifest_dir, &out_dir);
    compile_generated_rules(&manifest_dir, &out_dir);
}

fn compile_forbidden_functions(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/forbidden-functions.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read forbidden-functions rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawForbiddenRule> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
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
    let yaml_path = Path::new(manifest_dir).join("../../../rules/slow-functions.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read slow-functions rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawSlowRule> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
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

fn compile_native_methods(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/native-methods.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read native-methods rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawNativeMethod> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse native-methods rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/native-methods.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static NATIVE_METHODS: &[NativeMethodRule] = &[\n");
    for rule in &rules {
        generated.push_str(&format!(
            "    NativeMethodRule {{ object: {:?}, function: {:?} }},\n",
            rule.object, rule.function
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("native_methods_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

fn compile_actor_values(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/actor-values.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read actor-values rules at {}: {err}",
            yaml_path.display()
        )
    });
    let values: Vec<String> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse actor-values rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/actor-values.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static ACTOR_VALUES: &[&str] = &[\n");
    for value in &values {
        generated.push_str(&format!("    {value:?},\n"));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("actor_values_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

fn compile_update_event_pairs(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/update-event-handlers.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read update-event-handlers rules at {}: {err}",
            yaml_path.display()
        )
    });
    let pairs: Vec<RawUpdateEventPair> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse update-event-handlers rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/update-event-handlers.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static UPDATE_EVENT_PAIRS: &[UpdateEventPairRule] = &[\n");
    for pair in &pairs {
        generated.push_str(&format!(
            "    UpdateEventPairRule {{ register: {:?}, event: {:?} }},\n",
            pair.register, pair.event
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("update_event_pairs_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

fn compile_known_events(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/known-events.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read known-events rules at {}: {err}",
            yaml_path.display()
        )
    });
    let events: Vec<RawKnownEvent> = serde_norway::from_str(&yaml_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse known-events rules at {}: {err}",
            yaml_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str(
        "/// Compiled from `rules/known-events.yaml` by `build.rs`. Do not edit by hand.\n",
    );
    generated.push_str("pub static KNOWN_EVENTS: &[KnownEventRule] = &[\n");
    for event in &events {
        let mut args = String::new();
        for arg in &event.args {
            args.push_str(&format!(
                "EventArg {{ type_name: {:?}, name: {:?} }}, ",
                arg.type_name, arg.name
            ));
        }
        generated.push_str(&format!(
            "    KnownEventRule {{ event: {:?}, form: {:?}, args: &[{args}] }},\n",
            event.event, event.form
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("known_events_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

fn compile_rule_tags(manifest_dir: &str, out_dir: &str) {
    let json_path = Path::new(manifest_dir).join("../../../docs/rules.json");
    println!("cargo:rerun-if-changed={}", json_path.display());

    let json_src = fs::read_to_string(&json_path)
        .unwrap_or_else(|err| panic!("failed to read rule tags at {}: {err}", json_path.display()));
    let rules: Vec<RawRuleTag> = serde_json::from_str(&json_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse rule tags at {}: {err}",
            json_path.display()
        )
    });

    let mut generated = String::new();
    generated.push_str("/// Compiled from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("pub static RULE_TAGS: &[RuleTags] = &[\n");
    for rule in &rules {
        let importance = match rule.importance.as_str() {
            "low" => "Importance::Low",
            "medium" => "Importance::Medium",
            "high" => "Importance::High",
            other => panic!(
                "docs/rules.json: unknown importance `{other}` for {}",
                rule.id
            ),
        };
        if rule.tags.is_empty() {
            panic!("docs/rules.json: {} has no tags", rule.id);
        }
        let kinds = rule
            .tags
            .iter()
            .map(|t| format!("{t:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        generated.push_str(&format!(
            "    RuleTags {{ rule: {:?}, description: {:?}, kinds: &[{kinds}], importance: {importance} }},\n",
            rule.id, rule.definition
        ));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("rule_tags_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

/// Compiles `docs/rules.json` into `KNOWN_RULE_IDS`/`FIXABLE_RULE_IDS`
/// (`include!`d by `src/registry.rs`), in the JSON array's own order —
/// see this module's own doc comment.
fn compile_known_rule_ids(manifest_dir: &str, out_dir: &str) {
    let json_path = Path::new(manifest_dir).join("../../../docs/rules.json");
    println!("cargo:rerun-if-changed={}", json_path.display());

    let json_src = fs::read_to_string(&json_path)
        .unwrap_or_else(|err| panic!("failed to read rule ids at {}: {err}", json_path.display()));
    let rules: Vec<RawRuleId> = serde_json::from_str(&json_src)
        .unwrap_or_else(|err| panic!("failed to parse rule ids at {}: {err}", json_path.display()));

    let mut generated = String::new();
    generated.push_str("/// Compiled from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("pub const KNOWN_RULE_IDS: &[&str] = &[\n");
    for rule in &rules {
        generated.push_str(&format!("    {:?},\n", rule.id));
    }
    generated.push_str("];\n\n");
    generated.push_str("/// Compiled from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("pub const FIXABLE_RULE_IDS: &[&str] = &[\n");
    for rule in rules.iter().filter(|rule| rule.fixable) {
        generated.push_str(&format!("    {:?},\n", rule.id));
    }
    generated.push_str("];\n");

    let dest = Path::new(out_dir).join("known_rule_ids_data.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated rule data to {}: {err}",
            dest.display()
        )
    });
}

#[derive(serde::Deserialize)]
struct RawRuleMeta {
    id: String,
    name: String,
    fixable: bool,
    #[serde(default)]
    repair_order: Option<u32>,
    #[serde(default = "enabled_by_default_default")]
    enabled_by_default: bool,
}

fn enabled_by_default_default() -> bool {
    true
}

const RULE_ID_TO_MODULE: &[(&str, &str)] = &[
    ("float-to-int", "float_int_conversion"),
    ("unknown-actor-value", "actor_value"),
    ("event-signature-mismatch", "event_signature"),
    ("too-many-named-states", "too_many_states"),
    ("multiple-auto-states", "multiple_auto_states"),
];

/// `docs/rules.json` ids that do not become `Rules` field names by replacing
/// `-` with `_`. Keep in sync with `papyrus-lint-config/build.rs`.
const RULE_ID_TO_CONFIG_KEY: &[(&str, &str)] = &[
    ("float-to-int", "float_int_conversion"),
    ("too-many-named-states", "too_many_states"),
];

/// Project-level rules plus `unused-disable` (run from `lib.rs` after the
/// rest of the pass). These have no generated `check` call.
const NO_SOURCE_CHECK_IDS: &[&str] = &[
    "unused-disable",
    "conflicting-script-versions",
    "stale-compiled-output",
    "script-filename-mismatch",
];

/// Fixable in `docs/rules.json`, but applied through
/// `unused_import::repair_with` in `lib.rs` rather than `apply_repairs`.
const EXTERNAL_REPAIR_IDS: &[&str] = &["unused-import"];

fn config_key_for(id: &str) -> String {
    RULE_ID_TO_CONFIG_KEY
        .iter()
        .find(|(rule_id, _)| *rule_id == id)
        .map(|(_, key)| (*key).to_string())
        .unwrap_or_else(|| id.replace('-', "_"))
}

fn module_for_id(id: &str) -> String {
    RULE_ID_TO_MODULE
        .iter()
        .find(|(rule_id, _)| *rule_id == id)
        .map(|(_, module)| (*module).to_string())
        .unwrap_or_else(|| id.replace('-', "_"))
}

fn load_rule_meta(manifest_dir: &str) -> Vec<RawRuleMeta> {
    let json_path = Path::new(manifest_dir).join("../../../docs/rules.json");
    println!("cargo:rerun-if-changed={}", json_path.display());
    let json_src = fs::read_to_string(&json_path).unwrap_or_else(|err| {
        panic!(
            "failed to read rule metadata at {}: {err}",
            json_path.display()
        )
    });
    serde_json::from_str(&json_src).unwrap_or_else(|err| {
        panic!(
            "failed to parse rule metadata at {}: {err}",
            json_path.display()
        )
    })
}

fn validate_rules(rules: &[RawRuleMeta]) {
    let mut seen = std::collections::HashSet::new();
    for entry in rules {
        if !seen.insert(entry.id.as_str()) {
            panic!("docs/rules.json lists `{}` more than once", entry.id);
        }
        let no_source = NO_SOURCE_CHECK_IDS.contains(&entry.id.as_str());
        let external_repair = EXTERNAL_REPAIR_IDS.contains(&entry.id.as_str());
        if no_source && entry.repair_order.is_some() {
            panic!(
                "docs/rules.json: {} is a project/post-pass rule and must not have `repair_order`",
                entry.id
            );
        }
        if entry.repair_order.is_some() && !entry.fixable {
            panic!(
                "docs/rules.json: {} has `repair_order` but is not fixable",
                entry.id
            );
        }
        if entry.fixable && !external_repair && !no_source && entry.repair_order.is_none() {
            panic!(
                "docs/rules.json: {} is fixable and needs `repair_order` (or belong to EXTERNAL_REPAIR_IDS)",
                entry.id
            );
        }
        if external_repair && entry.repair_order.is_some() {
            panic!(
                "docs/rules.json: {} is repaired outside apply_repairs and must not have `repair_order`",
                entry.id
            );
        }
    }

    let mut orders: Vec<u32> = rules.iter().filter_map(|e| e.repair_order).collect();
    orders.sort();
    if !orders.is_empty() {
        let expected: Vec<u32> = (1..=orders.len() as u32).collect();
        if orders != expected {
            panic!(
                "docs/rules.json `repair_order` values must be 1..=N without gaps, got {orders:?}"
            );
        }
    }
}

fn default_yaml_rule_order(manifest_dir: &str) -> Vec<String> {
    let yaml_path = Path::new(manifest_dir).join("../../../docs/papyrus-lint.default.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());
    let text = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read default config at {}: {err}",
            yaml_path.display()
        )
    });
    let mut keys = Vec::new();
    let mut in_rules = false;
    for line in text.lines() {
        if line == "rules:" {
            in_rules = true;
            continue;
        }
        if !in_rules {
            continue;
        }
        if !line.starts_with("  ") || line.starts_with("   ") {
            break;
        }
        let Some((key, _)) = line.trim().split_once(':') else {
            break;
        };
        keys.push(key.to_string());
    }
    if keys.is_empty() {
        panic!("{} has no `rules:` entries", yaml_path.display());
    }
    keys
}

fn ordered_rule_meta<'a>(rules: &'a [RawRuleMeta], field_order: &[String]) -> Vec<&'a RawRuleMeta> {
    let mut by_key = std::collections::HashMap::new();
    for rule in rules {
        let key = config_key_for(&rule.id);
        if by_key.insert(key.clone(), rule).is_some() {
            panic!("duplicate Rules field `{key}`");
        }
    }
    let mut ordered = Vec::with_capacity(rules.len());
    for key in field_order {
        match by_key.remove(key) {
            Some(rule) => ordered.push(rule),
            None => panic!(
                "docs/papyrus-lint.default.yaml lists rules.{key} but docs/rules.json has no matching id"
            ),
        }
    }
    if !by_key.is_empty() {
        let mut missing: Vec<_> = by_key.keys().cloned().collect();
        missing.sort();
        panic!(
            "docs/papyrus-lint.default.yaml is missing rules: {missing:?}; add them next to the other `rules:` keys"
        );
    }
    ordered
}

fn compile_rules_struct(out_dir: &str, rules: &[&RawRuleMeta]) {
    let mut generated = String::new();
    generated.push_str("/// Individual enable/disable switches for each lint ruleset.\n");
    generated
        .push_str("/// Generated from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("///\n");
    generated.push_str("/// A ruleset set to `false` here is skipped by both\n");
    generated.push_str("/// [`crate::lint`]/[`crate::lint_with_external_arguments`] and, for\n");
    generated.push_str("/// rulesets with an automatic fix, [`crate::repair`]. Most rulesets\n");
    generated.push_str("/// default to `true`; those tagged `enabled_by_default: false` in\n");
    generated.push_str("/// `docs/rules.json` default to `false`.\n");
    generated.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
    generated.push_str("#[serde(default)]\n");
    generated.push_str("pub struct Rules {\n");
    for rule in rules {
        let key = config_key_for(&rule.id);
        let kind = if rule.fixable { "lint/fix" } else { "lint" };
        generated.push_str(&format!(
            "    /// The \"{}\" {kind}.\n",
            rule.name.replace('"', "\\\"")
        ));
        if !rule.enabled_by_default {
            generated.push_str("    /// Defaults to `false`.\n");
        }
        generated.push_str(&format!("    pub {key}: bool,\n"));
    }
    generated.push_str("}\n\n");
    generated.push_str("/// Default enable/disable flags for [`Rules`]. Generated from\n");
    generated.push_str("/// `docs/rules.json` (`enabled_by_default`, defaulting to `true`).\n");
    generated.push_str("pub fn default_rules() -> Rules {\n");
    generated.push_str("    Rules {\n");
    for rule in rules {
        let key = config_key_for(&rule.id);
        generated.push_str(&format!("        {key}: {},\n", rule.enabled_by_default));
    }
    generated.push_str("    }\n");
    generated.push_str("}\n");

    let dest = Path::new(out_dir).join("rules_struct.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated Rules struct to {}: {err}",
            dest.display()
        )
    });
}

/// Generates `collect_diagnostics` / `apply_repairs` from `docs/rules.json`.
fn compile_rules_dispatch(out_dir: &str, rules: &[RawRuleMeta]) {
    let mut modules = std::collections::BTreeSet::new();
    for entry in rules {
        if NO_SOURCE_CHECK_IDS.contains(&entry.id.as_str()) {
            continue;
        }
        modules.insert(module_for_id(&entry.id));
    }

    let mut generated = String::new();
    generated.push_str("use crate::{\n    ");
    let mut first = true;
    for m in &modules {
        if !first {
            generated.push_str(", ");
        }
        first = false;
        generated.push_str(m);
    }
    generated.push_str(", Diagnostic,\n};\n\n");

    generated.push_str("/// Runs every enabled source-level lint against `source`.\n");
    generated
        .push_str("/// Generated from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("#[allow(clippy::too_many_lines)]\n");
    generated.push_str("pub fn collect_diagnostics<E: ExternalSignatures>(\n");
    generated.push_str("    source: &str,\n");
    generated.push_str("    config: &Config,\n");
    generated.push_str("    external: &mut E,\n");
    generated.push_str(") -> Vec<Diagnostic> {\n");
    generated.push_str("    let tokens = papyrus_parser::tokenize(source).ok();\n");
    generated.push_str("    let tokens = tokens.as_deref();\n");
    generated.push_str("    let ast = papyrus_parser::parse(source).ok();\n");
    generated.push_str("    let ast = ast.as_ref();\n");
    generated.push_str("    let rules = &config.rules;\n");
    generated.push_str("    let mut diagnostics = Vec::new();\n");
    for entry in rules {
        if NO_SOURCE_CHECK_IDS.contains(&entry.id.as_str()) {
            continue;
        }
        let key = config_key_for(&entry.id);
        let module = module_for_id(&entry.id);
        generated.push_str(&format!("    if rules.{key} {{\n"));
        generated.push_str(&format!(
            "        diagnostics.extend({module}::check(source, ast, tokens, config, external));\n"
        ));
        generated.push_str("    }\n");
    }
    generated.push_str("    diagnostics\n");
    generated.push_str("}\n\n");

    generated
        .push_str("/// Applies every self-contained automatic fix whose ruleset is enabled.\n");
    generated
        .push_str("/// Generated from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("/// Repair order is `repair_order` in that file (not rule-id order),\n");
    generated.push_str("/// because later fixes see earlier rewrites.\n");
    generated.push_str("#[allow(clippy::too_many_lines)]\n");
    generated.push_str("pub fn apply_repairs(source: &str, config: &Config, applies: impl Fn(&str) -> bool) -> String {\n");
    generated.push_str("    let rules = &config.rules;\n");
    generated.push_str("    let mut source = source.to_string();\n");
    let mut repairs: Vec<&RawRuleMeta> =
        rules.iter().filter(|e| e.repair_order.is_some()).collect();
    repairs.sort_by_key(|e| e.repair_order.unwrap());
    if repairs.is_empty() {
        generated.push_str("    let _ = (rules, applies);\n");
    } else {
        for entry in &repairs {
            let key = config_key_for(&entry.id);
            let module = module_for_id(&entry.id);
            generated.push_str("    source = apply_rule(\n");
            generated.push_str("        source,\n");
            generated.push_str(&format!(
                "        rules.{key} && applies({module}::RULE),\n"
            ));
            generated.push_str(&format!(
                "        |source| {{\n            let tokens = papyrus_parser::tokenize(source).ok();\n            let ast = papyrus_parser::parse(source).ok();\n            {module}::repair(source, ast.as_ref(), tokens.as_deref(), config)\n        }},\n"
            ));
            generated.push_str("    );\n");
        }
    }
    generated.push_str("    source\n");
    generated.push_str("}\n");

    let dest = Path::new(out_dir).join("rules_dispatch.rs");
    fs::write(&dest, generated).unwrap_or_else(|err| {
        panic!(
            "failed to write generated dispatch to {}: {err}",
            dest.display()
        )
    });
}

fn compile_generated_rules(manifest_dir: &str, out_dir: &str) {
    let rules = load_rule_meta(manifest_dir);
    validate_rules(&rules);
    let field_order = default_yaml_rule_order(manifest_dir);
    let ordered = ordered_rule_meta(&rules, &field_order);
    compile_rules_struct(out_dir, &ordered);
    compile_rules_dispatch(out_dir, &rules);
}

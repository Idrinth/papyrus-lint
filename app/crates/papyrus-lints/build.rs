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
    doc_slug: String,
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
    let yaml_path = Path::new(manifest_dir).join("../../../rules/slow-functions.yaml");
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

fn compile_native_methods(manifest_dir: &str, out_dir: &str) {
    let yaml_path = Path::new(manifest_dir).join("../../../rules/native-methods.yaml");
    println!("cargo:rerun-if-changed={}", yaml_path.display());

    let yaml_src = fs::read_to_string(&yaml_path).unwrap_or_else(|err| {
        panic!(
            "failed to read native-methods rules at {}: {err}",
            yaml_path.display()
        )
    });
    let rules: Vec<RawNativeMethod> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
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
    let values: Vec<String> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
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
    let pairs: Vec<RawUpdateEventPair> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
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
    let events: Vec<RawKnownEvent> = serde_yaml::from_str(&yaml_src).unwrap_or_else(|err| {
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
            "    RuleTags {{ rule: {:?}, doc_slug: {:?}, description: {:?}, kinds: &[{kinds}], importance: {importance} }},\n",
            rule.id, rule.doc_slug, rule.definition
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
    let rules: Vec<RawRuleId> = serde_json::from_str(&json_src).unwrap_or_else(|err| {
        panic!("failed to parse rule ids at {}: {err}", json_path.display())
    });

    let mut generated = String::new();
    generated.push_str("/// Compiled from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
    generated.push_str("pub const KNOWN_RULE_IDS: &[&str] = &[\n");
    for rule in &rules {
        generated.push_str(&format!("    {:?},\n", rule.id));
    }
    generated.push_str("];\n\n");
    generated
        .push_str("/// Compiled from `docs/rules.json` by `build.rs`. Do not edit by hand.\n");
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

use super::BuildContext;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Deserialize)]
pub struct RuleMetadata {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub importance: String,
    pub definition: String,
    pub fixable: bool,
    /// How this rule would walk a script as a visitor: `"ast"` (parsed
    /// nodes), `"tokens"` (the lexer stream), or `"none"` (project-level,
    /// post-pass, or a raw source-line scan that is neither).
    pub visitor: String,
    #[serde(default)]
    pub repair_order: Option<u32>,
    #[serde(default = "enabled_by_default")]
    pub enabled_by_default: bool,
}

fn enabled_by_default() -> bool {
    true
}

const RULE_ID_TO_MODULE: &[(&str, &str)] = &[
    ("float-to-int", "float_int_conversion"),
    ("unknown-actor-value", "actor_value"),
    ("event-signature-mismatch", "event_signature"),
    ("too-many-named-states", "too_many_states"),
    ("multiple-auto-states", "multiple_auto_states"),
];

const RULE_ID_TO_CONFIG_KEY: &[(&str, &str)] = &[
    ("float-to-int", "float_int_conversion"),
    ("too-many-named-states", "too_many_states"),
];

pub const NO_SOURCE_CHECK_IDS: &[&str] = &[
    "unused-disable",
    "conflicting-script-versions",
    "stale-compiled-output",
    "script-filename-mismatch",
];
const EXTERNAL_REPAIR_IDS: &[&str] = &["unused-import", "argument-naming"];

#[derive(Debug, PartialEq, Eq)]
pub struct ValidationError(String);

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub fn load(context: &BuildContext) -> Vec<RuleMetadata> {
    context.load_json("shared/rules.json", "rule metadata")
}

pub fn config_key(id: &str) -> String {
    mapped_name(id, RULE_ID_TO_CONFIG_KEY)
}

pub fn module_name(id: &str) -> String {
    mapped_name(id, RULE_ID_TO_MODULE)
}

fn mapped_name(id: &str, overrides: &[(&str, &str)]) -> String {
    overrides
        .iter()
        .find(|(rule_id, _)| *rule_id == id)
        .map_or_else(|| id.replace('-', "_"), |(_, name)| (*name).to_string())
}

pub fn validate(rules: &[RuleMetadata]) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            return fail(format!(
                "shared/rules.json lists `{}` more than once",
                rule.id
            ));
        }
        if rule.tags.is_empty() {
            return fail(format!("shared/rules.json: {} has no tags", rule.id));
        }
        if !matches!(rule.importance.as_str(), "low" | "medium" | "high") {
            return fail(format!(
                "shared/rules.json: unknown importance `{}` for {}",
                rule.importance, rule.id
            ));
        }
        if !matches!(rule.visitor.as_str(), "ast" | "tokens" | "none") {
            return fail(format!(
                "shared/rules.json: unknown visitor `{}` for {} (expected ast, tokens, or none)",
                rule.visitor, rule.id
            ));
        }
        let no_source = NO_SOURCE_CHECK_IDS.contains(&rule.id.as_str());
        let external_repair = EXTERNAL_REPAIR_IDS.contains(&rule.id.as_str());
        if no_source && rule.repair_order.is_some() {
            return fail(format!(
                "shared/rules.json: {} is a project/post-pass rule and must not have `repair_order`",
                rule.id
            ));
        }
        if rule.repair_order.is_some() && !rule.fixable {
            return fail(format!(
                "shared/rules.json: {} has `repair_order` but is not fixable",
                rule.id
            ));
        }
        if rule.fixable && !external_repair && !no_source && rule.repair_order.is_none() {
            return fail(format!("shared/rules.json: {} is fixable and needs `repair_order` (or belong to EXTERNAL_REPAIR_IDS)", rule.id));
        }
        if external_repair && rule.repair_order.is_some() {
            return fail(format!("shared/rules.json: {} is repaired outside apply_repairs and must not have `repair_order`", rule.id));
        }
    }
    let mut orders: Vec<_> = rules.iter().filter_map(|rule| rule.repair_order).collect();
    orders.sort_unstable();
    let expected: Vec<_> = (1..=orders.len() as u32).collect();
    if orders != expected {
        return fail(format!(
            "shared/rules.json `repair_order` values must be 1..=N without gaps, got {orders:?}"
        ));
    }
    Ok(())
}

pub fn order_by_config<'a>(
    rules: &'a [RuleMetadata],
    field_order: &[String],
) -> Result<Vec<&'a RuleMetadata>, ValidationError> {
    let mut by_key = HashMap::new();
    for rule in rules {
        let key = config_key(&rule.id);
        if by_key.insert(key.clone(), rule).is_some() {
            return fail(format!("duplicate Rules field `{key}`"));
        }
    }
    let mut ordered = Vec::with_capacity(rules.len());
    for key in field_order {
        let Some(rule) = by_key.remove(key) else {
            return fail(format!("configuration/papyrus-lint.default.yaml lists rules.{key} but shared/rules.json has no matching id"));
        };
        ordered.push(rule);
    }
    if !by_key.is_empty() {
        let mut missing: Vec<_> = by_key.into_keys().collect();
        missing.sort();
        return fail(format!("configuration/papyrus-lint.default.yaml is missing rules: {missing:?}; add them next to the other `rules:` keys"));
    }
    Ok(ordered)
}

fn fail<T>(message: String) -> Result<T, ValidationError> {
    Err(ValidationError(message))
}

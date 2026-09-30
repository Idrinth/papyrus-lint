use super::BuildContext;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

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
    #[serde(default = "enabled_by_default")]
    pub enabled_by_default: bool,
    /// Optional allow-list of `Game::as_str()` values this rule runs for.
    /// Empty (the default when the field is omitted) means every game.
    /// [`papyrus_lint_globals::Game::Legacy`] also matches rules listed for
    /// `skyrim` at runtime.
    #[serde(default)]
    pub games: Vec<String>,
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

#[derive(Debug, Deserialize)]
pub struct RuleOrderFile {
    pub repair: Vec<String>,
    /// Fixable rules repaired outside `apply_repairs`.
    #[serde(default)]
    pub external_repair: Vec<String>,
}

pub fn load_rule_order(context: &BuildContext) -> RuleOrderFile {
    context.load_yaml::<RuleOrderFile>("shared/rule-order.yaml", "repair order")
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

pub fn validate(
    rules: &[RuleMetadata],
    repair_order: &[String],
    external_repair: &[String],
) -> Result<(), ValidationError> {
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
        for game in &rule.games {
            if papyrus_lint_globals::Game::from_str(game).is_err() {
                return fail(format!(
                    "shared/rules.json: unknown game `{game}` for {} (expected skyrim, legacy, fallout4, or starfield)",
                    rule.id
                ));
            }
        }
        let mut seen_games = HashSet::new();
        for game in &rule.games {
            if !seen_games.insert(game.as_str()) {
                return fail(format!(
                    "shared/rules.json: duplicate game `{game}` for {}",
                    rule.id
                ));
            }
        }
        let no_source = NO_SOURCE_CHECK_IDS.contains(&rule.id.as_str());
        let external = external_repair.iter().any(|id| id == rule.id.as_str());
        if no_source && repair_order.iter().any(|id| id == rule.id.as_str()) {
            return fail(format!(
                "shared/rule-order.yaml: {} is a project/post-pass rule and must not be listed under `repair`",
                rule.id
            ));
        }
        if external && repair_order.iter().any(|id| id == rule.id.as_str()) {
            return fail(format!(
                "shared/rule-order.yaml: {} is repaired outside apply_repairs and must not be listed under `repair`",
                rule.id
            ));
        }
        if rule.fixable
            && !external
            && !no_source
            && !repair_order.iter().any(|id| id == rule.id.as_str())
        {
            return fail(format!(
                "shared/rule-order.yaml: {} is fixable and must be listed under `repair` (or `external_repair`)",
                rule.id
            ));
        }
    }
    let mut seen_order = HashSet::new();
    let by_id: HashMap<_, _> = rules.iter().map(|rule| (rule.id.as_str(), rule)).collect();
    for id in repair_order {
        if !seen_order.insert(id.as_str()) {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` more than once under `repair`"
            ));
        }
        let Some(rule) = by_id.get(id.as_str()) else {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` under `repair` but shared/rules.json has no matching id"
            ));
        };
        if !rule.fixable {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` under `repair` but that rule is not fixable"
            ));
        }
    }
    let mut seen_external = HashSet::new();
    for id in external_repair {
        if !seen_external.insert(id.as_str()) {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` more than once under `external_repair`"
            ));
        }
        let Some(rule) = by_id.get(id.as_str()) else {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` under `external_repair` but shared/rules.json has no matching id"
            ));
        };
        if !rule.fixable {
            return fail(format!(
                "shared/rule-order.yaml lists `{id}` under `external_repair` but that rule is not fixable"
            ));
        }
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
            return fail(format!("shared/configuration/papyrus-lint.default.yaml lists rules.{key} but shared/rules.json has no matching id"));
        };
        ordered.push(rule);
    }
    if !by_key.is_empty() {
        let mut missing: Vec<_> = by_key.into_keys().collect();
        missing.sort();
        return fail(format!("shared/configuration/papyrus-lint.default.yaml is missing rules: {missing:?}; add them next to the other `rules:` keys"));
    }
    Ok(ordered)
}

fn fail<T>(message: String) -> Result<T, ValidationError> {
    Err(ValidationError(message))
}

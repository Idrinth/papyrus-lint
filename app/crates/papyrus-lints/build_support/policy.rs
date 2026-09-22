//! Hand-maintained lint policy tables loaded from
//! `shared/rules/data/{skyrim,fallout4}/`.
//!
//! Deprecated, forbidden, and slow function lists are **not** derived from
//! Creation Kit comments: those comments often sit next to the wrong
//! function. Actor values and update-event pairs are curated the same way.
//! Native methods, known events, and singleton globals live in
//! [`super::script_catalog`] instead.

use super::BuildContext;
use serde::Deserialize;

const GAMES: &[&str] = &["skyrim", "fallout4"];

#[derive(Clone, Debug, Deserialize)]
pub struct ForbiddenFunction {
    pub script: String,
    pub function: String,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub global: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeprecatedFunction {
    pub script: String,
    pub function: String,
    pub replacement: Option<String>,
    pub message: String,
    #[serde(default)]
    pub global: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlowFunction {
    pub object: String,
    pub function: String,
    pub replacement: String,
    #[serde(default)]
    pub global: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateEventPair {
    pub register: String,
    pub event: String,
}

pub fn forbidden_functions(context: &BuildContext) -> Vec<ForbiddenFunction> {
    load_merged(context, "forbidden-functions.yaml", "forbidden-functions rules")
}

pub fn deprecated_functions(context: &BuildContext) -> Vec<DeprecatedFunction> {
    load_merged(
        context,
        "deprecated-functions.yaml",
        "deprecated-functions rules",
    )
}

pub fn slow_functions(context: &BuildContext) -> Vec<SlowFunction> {
    load_merged(context, "slow-functions.yaml", "slow-functions rules")
}

pub fn actor_values(context: &BuildContext) -> Vec<String> {
    let mut seen = std::collections::BTreeMap::<String, String>::new();
    for game in GAMES {
        let relative = format!("shared/rules/data/{game}/actor-values.yaml");
        if !context.input(&relative).exists() {
            continue;
        }
        let values: Vec<String> = context.load_yaml(&relative, "actor-values rules");
        for value in values {
            seen.entry(value.to_ascii_lowercase()).or_insert(value);
        }
    }
    seen.into_values().collect()
}

pub fn update_event_pairs(context: &BuildContext) -> Vec<UpdateEventPair> {
    load_merged(
        context,
        "update-event-handlers.yaml",
        "update-event-handlers rules",
    )
}

fn load_merged<T>(context: &BuildContext, filename: &str, description: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned + Keyed,
{
    let mut seen = std::collections::BTreeMap::<String, T>::new();
    for game in GAMES {
        let relative = format!("shared/rules/data/{game}/{filename}");
        if !context.input(&relative).exists() {
            continue;
        }
        let values: Vec<T> = context.load_yaml(&relative, description);
        for value in values {
            seen.entry(value.key()).or_insert(value);
        }
    }
    seen.into_values().collect()
}

trait Keyed {
    fn key(&self) -> String;
}

impl Keyed for ForbiddenFunction {
    fn key(&self) -> String {
        format!(
            "{}.{}",
            self.script.to_ascii_lowercase(),
            self.function.to_ascii_lowercase()
        )
    }
}

impl Keyed for DeprecatedFunction {
    fn key(&self) -> String {
        format!(
            "{}.{}",
            self.script.to_ascii_lowercase(),
            self.function.to_ascii_lowercase()
        )
    }
}

impl Keyed for SlowFunction {
    fn key(&self) -> String {
        format!(
            "{}.{}",
            self.object.to_ascii_lowercase(),
            self.function.to_ascii_lowercase()
        )
    }
}

impl Keyed for UpdateEventPair {
    fn key(&self) -> String {
        format!(
            "{}.{}",
            self.register.to_ascii_lowercase(),
            self.event.to_ascii_lowercase()
        )
    }
}

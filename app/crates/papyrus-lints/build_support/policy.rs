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

pub fn forbidden_functions(context: &BuildContext, game: &str) -> Vec<ForbiddenFunction> {
    load_game(context, game, "forbidden-functions.yaml", "forbidden-functions rules")
}

pub fn deprecated_functions(context: &BuildContext, game: &str) -> Vec<DeprecatedFunction> {
    load_game(
        context,
        game,
        "deprecated-functions.yaml",
        "deprecated-functions rules",
    )
}

pub fn slow_functions(context: &BuildContext, game: &str) -> Vec<SlowFunction> {
    load_game(context, game, "slow-functions.yaml", "slow-functions rules")
}

pub fn actor_values(context: &BuildContext, game: &str) -> Vec<String> {
    let relative = format!("shared/rules/data/{game}/actor-values.yaml");
    if !context.input(&relative).exists() {
        return Vec::new();
    }
    context.load_yaml(&relative, "actor-values rules")
}

pub fn update_event_pairs(context: &BuildContext, game: &str) -> Vec<UpdateEventPair> {
    load_game(
        context,
        game,
        "update-event-handlers.yaml",
        "update-event-handlers rules",
    )
}

fn load_game<T>(context: &BuildContext, game: &str, filename: &str, description: &str) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
{
    let relative = format!("shared/rules/data/{game}/{filename}");
    if !context.input(&relative).exists() {
        return Vec::new();
    }
    context.load_yaml(&relative, description)
}

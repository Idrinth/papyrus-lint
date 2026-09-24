use papyrus_lints::repair;
use serde_json::Value;

use crate::documents::config_for_uri;

pub(crate) fn command_uri(params: &Value) -> Option<String> {
    let arguments = params.get("arguments")?.as_array()?;
    let argument = arguments.first()?;
    if let Some(uri) = argument.as_str() {
        return Some(uri.to_string());
    }
    argument["uri"]
        .as_str()
        .or_else(|| argument["textDocument"]["uri"].as_str())
        .map(str::to_string)
}

pub(crate) fn repaired_text(text: &str, uri: &str) -> Option<String> {
    let config = config_for_uri(uri);
    let repaired = repair(text, &config);
    (repaired != text).then_some(repaired)
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

use std::path::Path;

use papyrus_lints::{add_disable_comment, add_disable_file_comment, repaired_line, Config};
use serde_json::{json, Value};

use crate::documents::{config_for_uri, file_uri_to_path, Documents};

pub(crate) fn provide(documents: &Documents, params: &Value) -> Value {
    if !wants_quickfix(&params["context"]["only"]) {
        return json!([]);
    }
    let Some(uri) = params["textDocument"]["uri"].as_str() else {
        return json!([]);
    };
    let Some(document) = documents.get(uri) else {
        return json!([]);
    };
    let config = config_for_uri(uri);
    let diagnostics = relevant_diagnostics(params, &document.text, &config);
    let mut actions = Vec::new();
    for diagnostic in diagnostics {
        let Some(rule) = diagnostic["code"].as_str() else {
            continue;
        };
        let line = diagnostic["range"]["start"]["line"].as_u64().unwrap_or(0) as usize + 1;
        if let Some(fixed) = repaired_line(&document.text, &config, rule, line) {
            if let Some(updated) = replace_line(&document.text, line, &fixed) {
                actions.push(action(
                    &format!("Fix this issue ({rule})"),
                    &diagnostic,
                    replace_edit(uri, document.version, &document.text, &updated),
                ));
            }
        }
        if rule == "compiler-error" {
            continue;
        }
        let rules = vec![rule.to_string()];
        let ignored_line = add_disable_comment(&document.text, line, &rules);
        if ignored_line != document.text {
            actions.push(action(
                &format!("Ignore this lint for the line ({rule})"),
                &diagnostic,
                replace_edit(uri, document.version, &document.text, &ignored_line),
            ));
        }
        let ignored_file = add_disable_file_comment(&document.text, line, &rules);
        if ignored_file != document.text {
            actions.push(action(
                &format!("Ignore this lint for the file ({rule})"),
                &diagnostic,
                replace_edit(uri, document.version, &document.text, &ignored_file),
            ));
        }
        if let Some(project) = project_disable_edit(uri, rule) {
            actions.push(action(
                &format!("Ignore this lint for the project ({rule})"),
                &diagnostic,
                project,
            ));
        }
    }
    Value::Array(actions)
}

fn wants_quickfix(only: &Value) -> bool {
    let Some(kinds) = only.as_array() else {
        return true;
    };
    kinds.iter().any(|kind| {
        kind.as_str()
            .is_some_and(|kind| kind == "quickfix" || kind.starts_with("quickfix."))
    })
}

fn relevant_diagnostics(params: &Value, text: &str, config: &Config) -> Vec<Value> {
    if let Some(provided) = params["context"]["diagnostics"].as_array() {
        if !provided.is_empty() {
            return provided
                .iter()
                .filter(|diagnostic| {
                    diagnostic["source"] == "papyrus-lint" || diagnostic["code"].is_string()
                })
                .cloned()
                .collect();
        }
    }
    let start = params["range"]["start"]["line"].as_u64().unwrap_or(0);
    let end = params["range"]["end"]["line"].as_u64().unwrap_or(start);
    papyrus_lint_live::lint_source(text, config)
        .diagnostics
        .into_iter()
        .filter(|diagnostic| {
            let line = diagnostic.line.saturating_sub(1) as u64;
            (start..=end).contains(&line)
        })
        .map(|diagnostic| {
            crate::diagnostics::to_lsp(
                &diagnostic,
                crate::diagnostics::line_at(text, diagnostic.line),
            )
        })
        .collect()
}

fn action(title: &str, diagnostic: &Value, edit: Value) -> Value {
    json!({
        "title": title,
        "kind": "quickfix",
        "diagnostics": [diagnostic],
        "edit": edit,
    })
}

pub(crate) fn replace_edit(
    uri: &str,
    version: Option<i64>,
    original: &str,
    updated: &str,
) -> Value {
    json!({
        "documentChanges": [{
            "textDocument": { "uri": uri, "version": version },
            "edits": [{
                "range": full_range(original),
                "newText": updated,
            }]
        }]
    })
}

fn project_disable_edit(script_uri: &str, rule: &str) -> Option<Value> {
    let script = file_uri_to_path(script_uri)?;
    let dir = script.parent()?;
    let existing = ["papyrus-lint.yaml", "papyrus-lint.yml"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file());
    let path = existing.unwrap_or_else(|| dir.join("papyrus-lint.yaml"));
    let original = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = disable_rule_in_yaml(&original, rule);
    if updated == original {
        return None;
    }
    let uri = path_to_uri(&path);
    if path.is_file() {
        Some(replace_edit(&uri, None, &original, &updated))
    } else {
        Some(json!({
            "documentChanges": [
                { "kind": "create", "uri": uri },
                {
                    "textDocument": { "uri": uri },
                    "edits": [{
                        "range": full_range(""),
                        "newText": updated,
                    }]
                }
            ]
        }))
    }
}

pub(crate) fn disable_rule_in_yaml(yaml: &str, rule: &str) -> String {
    let key = rule.replace('-', "_");
    if key.is_empty() {
        return yaml.to_string();
    }
    let newline = if yaml.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = yaml
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect();
    if yaml.ends_with('\n') {
        lines.pop();
    }
    if let Some(block) = lines
        .iter()
        .position(|line| line.trim() == "rules:" || line.trim().starts_with("rules:#"))
    {
        let key_prefix = format!("{key}:");
        let mut insert_at = block + 1;
        let mut child_indent = "  ".to_string();
        for (offset, line) in lines.iter().enumerate().skip(block + 1) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                insert_at = offset + 1;
                continue;
            }
            if !line.starts_with([' ', '\t']) {
                break;
            }
            child_indent = line.chars().take_while(|ch| ch.is_whitespace()).collect();
            insert_at = offset + 1;
            if trimmed.starts_with(&key_prefix) {
                let mut replaced = line.clone();
                if let Some(colon) = replaced.find(':') {
                    let rest = &replaced[colon + 1..];
                    let comment = rest.find('#').map(|index| rest[index..].to_string());
                    replaced = format!(
                        "{}: false{}",
                        &replaced[..colon],
                        comment
                            .map(|comment| format!(" {comment}"))
                            .unwrap_or_default()
                    );
                }
                if replaced == *line {
                    return yaml.to_string();
                }
                lines[offset] = replaced;
                return join_lines(&lines, newline, yaml.ends_with('\n') || yaml.is_empty());
            }
        }
        lines.insert(insert_at, format!("{child_indent}{key}: false"));
        return join_lines(&lines, newline, true);
    }
    let mut body = yaml.to_string();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(&format!("rules:{newline}  {key}: false{newline}"));
    body
}

fn join_lines(lines: &[String], newline: &str, trailing: bool) -> String {
    let mut text = lines.join(newline);
    if trailing {
        text.push_str(newline);
    }
    text
}

fn full_range(text: &str) -> Value {
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len().saturating_sub(1);
    let end_text = lines.last().copied().unwrap_or("").trim_end_matches('\r');
    json!({
        "start": { "line": 0, "character": 0 },
        "end": {
            "line": last,
            "character": end_text.encode_utf16().count() as u32
        }
    })
}

fn replace_line(source: &str, line: usize, replacement: &str) -> Option<String> {
    let index = line.checked_sub(1)?;
    let mut lines: Vec<&str> = source.split('\n').collect();
    if source.ends_with('\n') {
        lines.pop();
    }
    let current = lines.get_mut(index)?;
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let cleaned = replacement.trim_end_matches(['\r', '\n']);
    *current = cleaned;
    let mut text = lines.join(newline);
    if source.ends_with('\n') {
        text.push_str(newline);
    }
    Some(text)
}

fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[cfg(test)]
#[path = "code_actions_tests.rs"]
mod tests;

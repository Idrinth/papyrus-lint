use papyrus_lints::Diagnostic;
use serde_json::{json, Value};

pub(crate) fn to_lsp(diagnostic: &Diagnostic, line_text: &str) -> Value {
    let start_chars = diagnostic.column.saturating_sub(1);
    let start = utf16_len_of_chars(line_text, start_chars);
    let end = utf16_len_of_chars(line_text, start_chars.saturating_add(1)).max(start);
    let line = diagnostic.line.saturating_sub(1) as u32;
    let severity = match diagnostic.level() {
        "warning" => 2,
        "info" => 3,
        _ => 1,
    };
    let mut lsp = json!({
        "range": {
            "start": { "line": line, "character": start },
            "end": { "line": line, "character": end }
        },
        "severity": severity,
        "source": "papyrus-lint",
        "message": display_message(&diagnostic.message),
        "code": diagnostic.rule,
    });
    if let Some(tags) = papyrus_lints::tags::tags_for(diagnostic.rule) {
        lsp["codeDescription"] = json!({ "href": tags.doc_url() });
    }
    lsp
}

fn display_message(message: &str) -> String {
    for prefix in ["[error] ", "[warning] ", "[info] "] {
        if let Some(rest) = message.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    message.to_string()
}

fn utf16_len_of_chars(text: &str, char_count: usize) -> u32 {
    text.chars()
        .take(char_count)
        .map(|ch| ch.len_utf16() as u32)
        .sum()
}

pub(crate) fn line_at(source: &str, line: usize) -> &str {
    source.lines().nth(line.saturating_sub(1)).unwrap_or("")
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;

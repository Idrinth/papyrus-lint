use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use papyrus_lint_config::load_config;
use papyrus_lints::{lint, Config};
use serde_json::{json, Value};

use crate::diagnostics::{line_at, to_lsp};
use crate::framing::write_message;

#[derive(Debug, Clone)]
pub(crate) struct OpenDocument {
    pub version: Option<i64>,
    pub text: String,
}

#[derive(Debug, Default)]
pub(crate) struct Documents {
    open: HashMap<String, OpenDocument>,
}

impl Documents {
    pub(crate) fn get(&self, uri: &str) -> Option<&OpenDocument> {
        self.open.get(uri)
    }

    pub(crate) fn replace_text(
        &mut self,
        uri: &str,
        text: String,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let Some(document) = self.open.get_mut(uri) else {
            return Ok(());
        };
        document.text.clone_from(&text);
        let version = document.version;
        self.publish(uri, version, &text, output)
    }

    pub(crate) fn did_open(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return Ok(());
        };
        let text = params["textDocument"]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let version = params["textDocument"]["version"].as_i64();
        self.open.insert(
            uri.to_string(),
            OpenDocument {
                version,
                text: text.clone(),
            },
        );
        self.publish(uri, version, &text, output)
    }

    pub(crate) fn did_change(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return Ok(());
        };
        let Some(document) = self.open.get_mut(uri) else {
            return Ok(());
        };
        if let Some(version) = params["textDocument"]["version"].as_i64() {
            document.version = Some(version);
        }
        if let Some(text) = params["contentChanges"]
            .as_array()
            .and_then(|changes| changes.last())
            .and_then(|change| change["text"].as_str())
        {
            document.text = text.to_string();
        }
        let version = document.version;
        let text = document.text.clone();
        self.publish(uri, version, &text, output)
    }

    pub(crate) fn did_save(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return Ok(());
        };
        if let Some(text) = params["text"].as_str() {
            if let Some(document) = self.open.get_mut(uri) {
                document.text = text.to_string();
            }
        }
        let Some(document) = self.open.get(uri) else {
            return Ok(());
        };
        let version = document.version;
        let text = document.text.clone();
        self.publish(uri, version, &text, output)
    }

    pub(crate) fn did_close(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return Ok(());
        };
        self.open.remove(uri);
        publish_diagnostics(output, uri, None, &[])
    }

    fn publish(
        &self,
        uri: &str,
        version: Option<i64>,
        text: &str,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let config = config_for_uri(uri);
        let diagnostics = lint(text, &config);
        let lsp = diagnostics
            .iter()
            .map(|diagnostic| to_lsp(diagnostic, line_at(text, diagnostic.line)))
            .collect::<Vec<_>>();
        publish_diagnostics(output, uri, version, &lsp)
    }
}

fn publish_diagnostics(
    output: &mut impl Write,
    uri: &str,
    version: Option<i64>,
    diagnostics: &[Value],
) -> io::Result<()> {
    let mut params = json!({
        "uri": uri,
        "diagnostics": diagnostics,
    });
    if let Some(version) = version {
        params["version"] = json!(version);
    }
    let body = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": params,
    }))
    .expect("publishDiagnostics json");
    write_message(output, &body)
}

pub(crate) fn config_for_uri(uri: &str) -> Config {
    let Some(path) = file_uri_to_path(uri) else {
        return Config::default();
    };
    let mut dir = path.parent();
    while let Some(current) = dir {
        if config_file(current).is_some() {
            return load_config(current).unwrap_or_else(|_| Config::default());
        }
        dir = current.parent();
    }
    Config::default()
}

fn config_file(dir: &Path) -> Option<PathBuf> {
    for name in ["papyrus-lint.yaml", "papyrus-lint.yml"] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("//localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(rest)?;
    Some(PathBuf::from(decoded))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;

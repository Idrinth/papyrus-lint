//! Fills the CLI help contact block from `shared/links.yaml` at compile
//! time. Entries are selected by the `contact` tag (the same filter
//! `<!--CONTACT-LINKS-->` uses elsewhere); labels come from the YAML keys
//! rather than being named here.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let links_path = manifest_dir.join("../../../shared/links.yaml");
    println!("cargo:rerun-if-changed={}", links_path.display());
    let source = fs::read_to_string(&links_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", links_path.display()));
    let rendered = render_plain_text(&source, "contact");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("contact_links.txt"), rendered)
        .unwrap_or_else(|error| panic!("failed to write generated contact links: {error}"));
}

fn render_plain_text(source: &str, tag: &str) -> String {
    let links = links_with_tag(source, tag);
    let width = links
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .expect("contact links");
    let mut rendered = String::new();
    for (label, url) in links {
        rendered.push_str(&format!("  {label:<width$}  {url}\n"));
    }
    rendered
}

fn links_with_tag(source: &str, tag: &str) -> Vec<(String, String)> {
    let mapping: serde_norway::Mapping = serde_norway::from_str(source)
        .unwrap_or_else(|error| panic!("failed to parse shared/links.yaml: {error}"));
    let mut links = Vec::new();
    for (key, value) in mapping {
        let label = key
            .as_str()
            .unwrap_or_else(|| panic!("shared/links.yaml: labels must be strings"))
            .to_string();
        let url = value
            .get("url")
            .and_then(serde_norway::Value::as_str)
            .unwrap_or_else(|| panic!("shared/links.yaml: {label} is missing url"))
            .to_string();
        let tags = value.get("type").and_then(serde_norway::Value::as_sequence);
        let Some(tags) = tags else {
            panic!("shared/links.yaml: {label} is missing type");
        };
        if tags.iter().any(|entry| entry.as_str() == Some(tag)) {
            links.push((label, url));
        }
    }
    if links.is_empty() {
        panic!("shared/links.yaml has no entries tagged `{tag}`");
    }
    links
}

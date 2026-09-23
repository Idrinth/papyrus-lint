//! Build-time catalog of native methods, engine events, and singleton
//! scripts extracted from the bundled Creation Kit / script-extender
//! archives under `shared/scripts`.
//!
//! This module is `include!`d (via `#[path]`) from more than one crate's
//! `build.rs`, so it must not depend on other `build_support` modules.
//! Each crate only calls a subset of the catalog helpers; allow unused
//! items rather than duplicating the parser.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub const GAMES: &[&str] = &["skyrim", "fallout4"];

/// Forms whose event declarations win when the same event name appears on
/// more than one script in a single game archive. Lower index is walked
/// first so a later overlay cannot replace the preferred Form.
const EVENT_FORM_PRIORITY: &[&str] = &[
    "ScriptObject",
    "Form",
    "ObjectReference",
    "Actor",
    "ActiveMagicEffect",
    "Alias",
    "ReferenceAlias",
    "LocationAlias",
    "Quest",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMethod {
    pub object: String,
    pub function: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventArg {
    pub type_name: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownEvent {
    pub event: String,
    pub form: String,
    pub args: Vec<EventArg>,
}

struct ScriptHeader {
    name: String,
    hidden: bool,
    native: bool,
    natives: Vec<String>,
    events: Vec<(String, Vec<EventArg>)>,
    has_instance_native: bool,
    has_global_native: bool,
}

fn base_archive(game: &str) -> &'static str {
    match game {
        "fallout4" => "fallout4-scripts.zip",
        _ => "skyrim-scripts.zip",
    }
}

fn extender_archive(game: &str) -> &'static str {
    match game {
        "fallout4" => "fallout4-extender-scripts.zip",
        _ => "skyrim-extender-scripts.zip",
    }
}

/// Lowercased singleton script names (`Game`, `Utility`, F4SE `UI`, …)
/// referenced by literal type name rather than through a typed variable.
pub fn native_global_names(scripts_dir: &Path, game: &str) -> Vec<String> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for archive in [base_archive(game), extender_archive(game)] {
        for script in parse_archive(scripts_dir, archive) {
            if is_singleton(&script) {
                names
                    .entry(script.name.to_ascii_lowercase())
                    .or_insert(script.name.to_ascii_lowercase());
            }
        }
    }
    names.into_values().collect()
}

/// Base-game `Native` functions from the vanilla (non-extender) archive.
pub fn native_methods(scripts_dir: &Path, game: &str) -> Vec<NativeMethod> {
    let mut seen: BTreeMap<(String, String), NativeMethod> = BTreeMap::new();
    for script in parse_archive(scripts_dir, base_archive(game)) {
        for function in script.natives {
            let key = (
                script.name.to_ascii_lowercase(),
                function.to_ascii_lowercase(),
            );
            seen.entry(key).or_insert(NativeMethod {
                object: script.name.clone(),
                function,
            });
        }
    }
    seen.into_values().collect()
}

/// Engine `Event` signatures from Hidden base-game scripts for `game`.
///
/// `OnInit` is injected when no Hidden header declares it, matching the
/// curated Skyrim table.
pub fn known_events(scripts_dir: &Path, game: &str) -> Vec<KnownEvent> {
    let mut by_name: BTreeMap<String, KnownEvent> = BTreeMap::new();
    let mut scripts = parse_archive(scripts_dir, base_archive(game));
    scripts.sort_by_key(|script| event_form_rank(&script.name));
    for script in scripts {
        if !script.hidden {
            continue;
        }
        for (event, args) in script.events {
            by_name
                .entry(event.to_ascii_lowercase())
                .or_insert(KnownEvent {
                    event,
                    form: script.name.clone(),
                    args,
                });
        }
    }
    by_name.into_values().collect()
}

fn event_form_rank(name: &str) -> usize {
    EVENT_FORM_PRIORITY
        .iter()
        .position(|form| form.eq_ignore_ascii_case(name))
        .unwrap_or(EVENT_FORM_PRIORITY.len())
}

fn is_singleton(script: &ScriptHeader) -> bool {
    script.has_global_native && !script.has_instance_native
}

fn parse_archive(scripts_dir: &Path, archive_name: &str) -> Vec<ScriptHeader> {
    let path = scripts_dir.join(archive_name);
    println!("cargo:rerun-if-changed={}", path.display());
    if !path.exists() {
        return Vec::new();
    }
    let file = File::open(&path).unwrap_or_else(|err| {
        panic!(
            "failed to open bundled scripts at {}: {err}",
            path.display()
        )
    });
    let mut archive = zip::ZipArchive::new(file).unwrap_or_else(|err| {
        panic!(
            "failed to read bundled scripts zip at {}: {err}",
            path.display()
        )
    });
    let mut scripts = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap_or_else(|err| {
            panic!(
                "failed to read zip entry {i} from {}: {err}",
                path.display()
            )
        });
        let name = entry.name().to_string();
        if !name.to_ascii_lowercase().ends_with(".psc") {
            continue;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .unwrap_or_else(|err| panic!("failed to read {name} from {}: {err}", path.display()));
        let source = String::from_utf8_lossy(&bytes);
        if let Some(script) = parse_script(&source) {
            scripts.push(script);
        }
    }
    scripts
}

fn parse_script(source: &str) -> Option<ScriptHeader> {
    let lines = logical_lines(source);
    let header = lines.iter().find(|line| {
        starts_with_ignore_ascii_case(line, "Scriptname ")
            || starts_with_ignore_ascii_case(line, "ScriptName ")
    })?;
    let (name, hidden, native) = parse_scriptname(header)?;
    let mut natives = Vec::new();
    let mut events = Vec::new();
    let mut has_instance_native = false;
    let mut has_global_native = false;
    for line in &lines {
        if let Some((function, flags)) = parse_function_line(line) {
            let is_native = flag_set(&flags, "Native");
            let is_global = flag_set(&flags, "Global");
            if is_native {
                natives.push(function);
                if is_global {
                    has_global_native = true;
                } else {
                    has_instance_native = true;
                }
            }
        } else if let Some((event, args)) = parse_event_line(line) {
            events.push((event, args));
        }
    }
    Some(ScriptHeader {
        name,
        hidden,
        native,
        natives,
        events,
        has_instance_native,
        has_global_native,
    })
}

fn parse_scriptname(line: &str) -> Option<(String, bool, bool)> {
    let rest = strip_prefix_ignore_ascii_case(line, "Scriptname ")?;
    let mut parts = rest.split_whitespace();
    let name = parts.next()?.to_string();
    let flags: Vec<&str> = parts
        .filter(|part| !part.eq_ignore_ascii_case("extends"))
        .filter(|part| {
            !part.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                || part.eq_ignore_ascii_case("Native")
                || part.eq_ignore_ascii_case("Hidden")
                || part.eq_ignore_ascii_case("Conditional")
        })
        .collect();
    // `Extends Foo` consumes two tokens; treat remaining tokens as flags.
    let upper = rest.to_ascii_lowercase();
    let hidden = upper.split_whitespace().any(|t| t == "hidden");
    let native = upper.split_whitespace().any(|t| t == "native");
    let _ = flags;
    Some((name, hidden, native))
}

fn parse_function_line(line: &str) -> Option<(String, String)> {
    let lowered = line.to_ascii_lowercase();
    let idx = lowered.find("function ")?;
    // Ignore `EndFunction`.
    if lowered[..idx].contains("end") {
        return None;
    }
    let after = line[idx + "function ".len()..].trim_start();
    let name_end = after.find('(')?;
    let name = after[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    let close = after.rfind(')')?;
    let flags = after[close + 1..].trim().to_string();
    Some((name.to_string(), flags))
}

fn parse_event_line(line: &str) -> Option<(String, Vec<EventArg>)> {
    let lowered = line.to_ascii_lowercase();
    let idx = lowered.find("event ")?;
    if lowered[..idx].contains("end") {
        return None;
    }
    let after = line[idx + "event ".len()..].trim_start();
    let name_end = after.find('(')?;
    let name = after[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    let close = after.rfind(')')?;
    let params = after[name_end + 1..close].trim();
    Some((name.to_string(), parse_params(params)))
}

fn parse_params(params: &str) -> Vec<EventArg> {
    if params.trim().is_empty() {
        return Vec::new();
    }
    params
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let without_default = part.split('=').next().unwrap_or(part).trim();
            let mut tokens = without_default.split_whitespace();
            let type_name = tokens.next()?.trim_end_matches("[]").to_string();
            let name = tokens.next().unwrap_or("").to_string();
            Some(EventArg { type_name, name })
        })
        .collect()
}

fn flag_set(flags: &str, wanted: &str) -> bool {
    flags
        .split_whitespace()
        .any(|flag| flag.eq_ignore_ascii_case(wanted))
}

fn logical_lines(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for raw in source.lines() {
        let without_comment = strip_line_comment(raw);
        let trimmed = without_comment.trim_end();
        if trimmed.ends_with('\\') {
            buf.push_str(trimmed.trim_end_matches('\\').trim_end());
            buf.push(' ');
            continue;
        }
        buf.push_str(trimmed);
        let joined = buf.trim().to_string();
        if !joined.is_empty() {
            out.push(joined);
        }
        buf.clear();
    }
    if !buf.trim().is_empty() {
        out.push(buf.trim().to_string());
    }
    out
}

fn strip_line_comment(line: &str) -> &str {
    let mut in_string = false;
    for (idx, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            ';' if !in_string => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn strip_prefix_ignore_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    starts_with_ignore_ascii_case(value, prefix).then(|| &value[prefix.len()..])
}

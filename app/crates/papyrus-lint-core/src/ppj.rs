//! Parser for `.ppj` (Papyrus Project) files: the XML project format used by
//! Caprica and Bethesda's own `PapyrusCompiler.exe`/Creation Kit tooling to
//! describe a mod's import search paths and the scripts (or whole folders of
//! scripts) it compiles, e.g.:
//!
//! ```xml
//! <PapyrusProject xmlns="PapyrusProject.xsd" Flags="TESV_Papyrus_Flags.flg"
//!     Game="sse" Output="Scripts">
//!     <Imports>
//!         <Import>.\Source\Scripts</Import>
//!         <Import>C:\...\Skyrim Special Edition\Data\Source\Scripts</Import>
//!     </Imports>
//!     <Folders>
//!         <Folder>.\Source\Scripts</Folder>
//!     </Folders>
//!     <Scripts>
//!         <Script>MyMod:MyQuestScript</Script>
//!     </Scripts>
//! </PapyrusProject>
//! ```
//!
//! A project's own scripts are declared either as `<Folder>` entries (every
//! `.psc` found under that directory, recursively unless `NoRecurse="true"`)
//! or `<Script>` entries (a dotted Papyrus object name, e.g. `MyMod:MyQuest`,
//! resolved against `<Import>` the same way the compiler resolves an
//! `Extends`). `<Import>` entries are also the project's own
//! `additional_script_roots` equivalent: passed to the real compiler's `-i`
//! argument, and needed here to resolve `<Script>` entries and cross-script
//! lookups (a base game's own vanilla scripts, for one, are conventionally
//! listed as an `<Import>` without being part of the project's own
//! `<Folders>`/`<Scripts>`).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug)]
pub enum PpjError {
    Io(std::io::Error),
    Xml(roxmltree::Error),
    /// The document parsed as XML but its root element isn't
    /// `PapyrusProject`.
    NotAPapyrusProject,
}

impl fmt::Display for PpjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PpjError::Io(err) => write!(f, "failed to read ppj file: {err}"),
            PpjError::Xml(err) => write!(f, "failed to parse ppj file: {err}"),
            PpjError::NotAPapyrusProject => {
                write!(
                    f,
                    "failed to parse ppj file: root element is not <PapyrusProject>"
                )
            }
        }
    }
}

impl std::error::Error for PpjError {}

impl From<std::io::Error> for PpjError {
    fn from(err: std::io::Error) -> Self {
        PpjError::Io(err)
    }
}

impl From<roxmltree::Error> for PpjError {
    fn from(err: roxmltree::Error) -> Self {
        PpjError::Xml(err)
    }
}

/// A `.ppj` file's contents, resolved against the directory it lives in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PpjProject {
    /// `<Import>` entries, in document order, resolved relative to the ppj
    /// file's own directory (an already-absolute entry, including a
    /// Windows-style `C:\...`/UNC path given on any host OS, is kept as-is,
    /// with any `\` normalized to `/`; see [`normalize_separators`]). This
    /// is the project's `additional_script_roots` equivalent: it's what a
    /// real compile passes as `-i`, and what `<Script>` entries below are
    /// resolved against.
    pub imports: Vec<PathBuf>,
    /// Every `.psc` file the project compiles: each `<Folder>` entry's own
    /// `.psc` files (recursively, unless that folder sets
    /// `NoRecurse="true"`), followed by each `<Script>` entry resolved
    /// against `imports` (falling back to the first import, or the ppj's own
    /// directory if it has none, when no import actually contains it — so a
    /// script that can't be resolved still surfaces as a missing-file error
    /// downstream instead of silently vanishing).
    pub scripts: Vec<PathBuf>,
    /// The `Output` attribute, if set, resolved the same way as an
    /// `<Import>`.
    pub output: Option<PathBuf>,
    /// The `Flags` attribute, kept as written (it names a `.flg` file
    /// resolved by the compiler itself, not a path this crate resolves).
    pub flags: Option<String>,
    /// The `Game` attribute (`sse`, `tesv`, or `fo4`), if set.
    pub game: Option<String>,
}

/// Parses the `.ppj` file at `ppj_path`.
pub fn parse_ppj(ppj_path: &Path) -> Result<PpjProject, PpjError> {
    let contents = fs::read_to_string(ppj_path)?;
    let doc = roxmltree::Document::parse(&contents)?;
    let root = doc.root_element();
    if root.tag_name().name() != "PapyrusProject" {
        return Err(PpjError::NotAPapyrusProject);
    }

    let base_dir = ppj_path.parent().unwrap_or_else(|| Path::new(""));

    let imports: Vec<PathBuf> = root
        .children()
        .find(|node| node.has_tag_name("Imports"))
        .into_iter()
        .flat_map(|imports| imports.children())
        .filter(|node| node.has_tag_name("Import"))
        .filter_map(element_text)
        .map(|import| resolve_ppj_path(base_dir, &import))
        .collect();

    let mut scripts = Vec::new();
    if let Some(folders) = root.children().find(|node| node.has_tag_name("Folders")) {
        for folder in folders
            .children()
            .filter(|node| node.has_tag_name("Folder"))
        {
            let Some(text) = element_text(folder) else {
                continue;
            };
            let no_recurse = folder
                .attribute("NoRecurse")
                .is_some_and(|value| value.eq_ignore_ascii_case("true"));
            let dir = resolve_ppj_path(base_dir, &text);
            scripts.extend(find_psc_files(&dir, !no_recurse));
        }
    }
    if let Some(script_list) = root.children().find(|node| node.has_tag_name("Scripts")) {
        for script in script_list
            .children()
            .filter(|node| node.has_tag_name("Script"))
        {
            let Some(text) = element_text(script) else {
                continue;
            };
            scripts.push(resolve_script_object_name(base_dir, &imports, &text));
        }
    }

    let output = root
        .attribute("Output")
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_ppj_path(base_dir, value));
    let flags = root
        .attribute("Flags")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let game = root
        .attribute("Game")
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);

    Ok(PpjProject {
        imports,
        scripts,
        output,
        flags,
        game,
    })
}

/// An element's own direct text content, trimmed. `None` if it has no text
/// (or is only whitespace), matching a self-closing/empty `<Import/>` etc.
fn element_text(node: roxmltree::Node) -> Option<String> {
    let text = node.text()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Replaces every literal `\` with `/`. A ppj file is conventionally
/// authored on Windows, so its paths use `\` throughout — including its own
/// project-local `<Folder>`/`<Import>` entries, which (unlike a vendor path
/// pointing outside the project) must actually resolve to a real, scannable
/// directory for this crate to do anything useful. `\` isn't a path
/// separator on a non-Windows host, so it's normalized to `/` up front,
/// which every host OS (Windows included) accepts as a separator.
fn normalize_separators(entry: &str) -> String {
    entry.replace('\\', "/")
}

/// Whether `entry` (already [`normalize_separators`]d) is an absolute path
/// on any host OS: a POSIX `/...` path, a Windows drive-letter path
/// (`C:/...`), or a normalized UNC path (`//server/share`).
/// `Path::is_absolute` alone isn't enough here since a host OS other than
/// Windows doesn't recognize a drive-letter path as absolute.
fn is_absolute_anywhere(entry: &str) -> bool {
    if Path::new(entry).is_absolute() || entry.starts_with('/') {
        return true;
    }
    let mut chars = entry.chars();
    matches!(chars.next(), Some(letter) if letter.is_ascii_alphabetic())
        && chars.next() == Some(':')
        && chars.next() == Some('/')
}

/// Resolves an `<Import>`/`<Folder>`/`Output` path against `base_dir` (the
/// ppj file's own directory): normalized (see [`normalize_separators`]),
/// then kept as-is if already absolute (see [`is_absolute_anywhere`]),
/// otherwise joined onto `base_dir`.
fn resolve_ppj_path(base_dir: &Path, entry: &str) -> PathBuf {
    let normalized = normalize_separators(entry);
    if is_absolute_anywhere(&normalized) {
        PathBuf::from(normalized)
    } else {
        base_dir.join(normalized)
    }
}

/// Resolves a `<Script>` entry (a dotted Papyrus object name, e.g.
/// `MyMod:MyQuestScript`, with `:` separating namespace components the same
/// way a directory separator would) against `imports` in order, returning
/// the first import under which the resulting `.psc` actually exists.
/// Falls back to the first import (or `base_dir` if there are none) so an
/// entry that doesn't resolve anywhere still yields a path — surfacing as a
/// missing-file error downstream instead of silently dropping the entry.
fn resolve_script_object_name(base_dir: &Path, imports: &[PathBuf], name: &str) -> PathBuf {
    let relative: PathBuf = name.split(':').collect();
    let relative = relative.with_extension("psc");

    imports
        .iter()
        .map(|import| import.join(&relative))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| {
            imports
                .first()
                .cloned()
                .unwrap_or_else(|| base_dir.to_path_buf())
                .join(&relative)
        })
}

/// Every `.psc` file under `dir`, matched case-insensitively on extension,
/// in sorted order. Recurses into subdirectories when `recurse` is `true`
/// (the default, absent an explicit `NoRecurse="true"` on the `<Folder>`
/// entry); otherwise only `dir`'s own immediate files are considered. An
/// unreadable/missing `dir` yields no entries.
fn find_psc_files(dir: &Path, recurse: bool) -> Vec<PathBuf> {
    let mut walker = WalkDir::new(dir).follow_links(true);
    if !recurse {
        walker = walker.max_depth(1);
    }
    let mut results: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            !path.is_dir()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
        })
        .collect();
    results.sort();
    results
}

#[cfg(test)]
#[path = "ppj_tests.rs"]
mod tests;

//! On-disk index of script content hashes used by
//! `conflicting-script-versions`.
//!
//! Each file lives next to the AST cache as
//! `{game}-{sha256(lowercase-filename)}.iplcc` and holds every
//! known implementer of that script name: absolute path, mtime, and a
//! SHA-256 of the decoded source. A still-fresh implementer lets a later
//! run compare copies without opening the `.psc`.
//!
//! The on-disk document is an internal binary layout (magic `IPLC`, a
//! little-endian format version, then bincode of the collision groups).
//! It is not a public interchange format.
//!
//! Callers preload names they already know, record hashes while source is
//! in memory (the parse phase), and [`flush`] dirty files at parse-end.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use papyrus_lint_globals::Game;
use serde::{Deserialize, Serialize};

use papyrus_ast_cache as ast_cache;
use sha2::{Digest, Sha256};

pub const ALGORITHM: &str = "sha256";
const MAGIC: &[u8; 4] = b"IPLC";
const FORMAT_VERSION: u32 = 1;

/// One on-disk implementer of a script name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Implementer {
    pub hash: String,
    pub algorithm: String,
    pub mtime: String,
    pub path: String,
}

/// Every known copy of one script file name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCollisions {
    pub scriptname: String,
    pub implementers: Vec<Implementer>,
}

type GroupKey = (PathBuf, String, String);

struct Store {
    groups: HashMap<GroupKey, ScriptCollisions>,
    loaded: HashSet<GroupKey>,
    dirty: HashSet<GroupKey>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(Store {
            groups: HashMap::new(),
            loaded: HashSet::new(),
            dirty: HashSet::new(),
        })
    })
}

fn lock_store() -> std::sync::MutexGuard<'static, Store> {
    store()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn group_key(dir: &Path, game: Game, scriptname: &str) -> GroupKey {
    (
        dir.to_path_buf(),
        game.as_str().to_string(),
        scriptname.to_ascii_lowercase(),
    )
}

fn collisions_path(dir: &Path, game: Game, scriptname: &str) -> PathBuf {
    let digest = sha256_bytes(scriptname.to_ascii_lowercase().as_bytes());
    dir.join(format!("{}-{digest}.iplcc", game.as_str()))
}

fn file_mtime_secs(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

fn stored_path(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn paths_match(stored: &str, path: &Path) -> bool {
    let stored_path = Path::new(stored);
    if stored_path == path {
        return true;
    }
    match (
        std::fs::canonicalize(stored_path),
        std::fs::canonicalize(path),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => stored_path.to_string_lossy() == path.to_string_lossy(),
    }
}

fn load_group(store: &mut Store, dir: &Path, game: Game, scriptname: &str) {
    let key = group_key(dir, game, scriptname);
    if !store.loaded.insert(key.clone()) {
        return;
    }
    let Ok(raw) = std::fs::read(collisions_path(dir, game, scriptname)) else {
        return;
    };
    let Ok(groups) = decode_collisions(&raw) else {
        return;
    };
    for group in groups {
        let scriptname = group.scriptname.to_ascii_lowercase();
        store.groups.insert(
            group_key(dir, game, &scriptname),
            ScriptCollisions {
                scriptname,
                implementers: group.implementers,
            },
        );
    }
}

fn upsert_implementer(
    store: &mut Store,
    dir: &Path,
    game: Game,
    path: &Path,
    hash: &str,
    mtime: u64,
) {
    let Some(scriptname) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let scriptname = scriptname.to_ascii_lowercase();
    load_group(store, dir, game, &scriptname);
    let key = group_key(dir, game, &scriptname);
    let stored_path = stored_path(path);
    let group = store
        .groups
        .entry(key.clone())
        .or_insert_with(|| ScriptCollisions {
            scriptname: scriptname.clone(),
            implementers: Vec::new(),
        });
    if let Some(existing) = group
        .implementers
        .iter_mut()
        .find(|implementer| paths_match(&implementer.path, path))
    {
        if existing.hash == hash
            && existing.algorithm == ALGORITHM
            && existing.mtime == mtime.to_string()
            && existing.path == stored_path
        {
            return;
        }
        existing.hash = hash.to_string();
        existing.algorithm = ALGORITHM.to_string();
        existing.mtime = mtime.to_string();
        existing.path = stored_path;
    } else {
        group.implementers.push(Implementer {
            hash: hash.to_string(),
            algorithm: ALGORITHM.to_string(),
            mtime: mtime.to_string(),
            path: stored_path,
        });
        group
            .implementers
            .sort_by(|left, right| left.path.cmp(&right.path));
    }
    store.dirty.insert(key);
}

fn cached_hash(store: &mut Store, dir: &Path, game: Game, path: &Path) -> Option<String> {
    let scriptname = path.file_name()?.to_str()?;
    load_group(store, dir, game, scriptname);
    let mtime = file_mtime_secs(path)?.to_string();
    let group = store.groups.get(&group_key(dir, game, scriptname))?;
    group.implementers.iter().find_map(|implementer| {
        if implementer.algorithm == ALGORITHM
            && implementer.mtime == mtime
            && paths_match(&implementer.path, path)
        {
            Some(implementer.hash.clone())
        } else {
            None
        }
    })
}

/// Loads collision files for every distinct file name in `paths`.
pub fn preload(game: Game, paths: impl IntoIterator<Item = impl AsRef<Path>>) {
    let Some(dir) = ast_cache::cache_dir() else {
        return;
    };
    preload_in(&dir, game, paths);
}

pub(crate) fn preload_in(
    dir: &Path,
    game: Game,
    paths: impl IntoIterator<Item = impl AsRef<Path>>,
) {
    game.assert_supported();
    let mut names = HashSet::new();
    for path in paths {
        if let Some(name) = path.as_ref().file_name().and_then(|name| name.to_str()) {
            names.insert(name.to_ascii_lowercase());
        }
    }
    let mut store = lock_store();
    for name in names {
        load_group(&mut store, dir, game, &name);
    }
}

/// Records `source`'s SHA-256 against `path`'s current mtime so a later
/// lookup does not need to reopen the file.
pub fn remember_source(game: Game, path: &Path, source: &str) {
    let Some(dir) = ast_cache::cache_dir() else {
        return;
    };
    remember_source_in(&dir, game, path, source);
}

pub(crate) fn remember_source_in(dir: &Path, game: Game, path: &Path, source: &str) {
    game.assert_supported();
    let Some(mtime) = file_mtime_secs(path) else {
        return;
    };
    let mut store = lock_store();
    upsert_implementer(&mut store, dir, game, path, &sha256_hex(source), mtime);
}

/// SHA-256 of `path`'s decoded contents, from a still-fresh collision
/// entry when one exists, otherwise by reading the file and recording the
/// result.
pub fn content_hash_for(game: Game, path: &Path) -> Option<String> {
    content_hash_in(ast_cache::cache_dir()?.as_path(), game, path)
}

pub(crate) fn content_hash_in(dir: &Path, game: Game, path: &Path) -> Option<String> {
    game.assert_supported();
    {
        let mut store = lock_store();
        if let Some(hash) = cached_hash(&mut store, dir, game, path) {
            return Some(hash);
        }
    }
    let contents = std::fs::read(path).ok()?;
    let source = decode_psc_source(&contents);
    let hash = sha256_hex(&source);
    if let Some(mtime) = file_mtime_secs(path) {
        let mut store = lock_store();
        upsert_implementer(&mut store, dir, game, path, &hash, mtime);
    }
    Some(hash)
}

/// Writes every dirty collision file under the active cache directory.
pub fn flush() {
    let Some(dir) = ast_cache::cache_dir() else {
        return;
    };
    flush_in(&dir);
}

pub(crate) fn flush_in(dir: &Path) {
    let mut store = lock_store();
    let dirty: Vec<GroupKey> = store.dirty.drain().collect();
    if dirty.is_empty() {
        return;
    }
    if std::fs::create_dir_all(dir).is_err() {
        store.dirty.extend(dirty);
        return;
    }
    for key in dirty {
        let Some(group) = store.groups.get(&key) else {
            continue;
        };
        if key.0 != dir {
            store.dirty.insert(key);
            continue;
        }
        let path = collisions_path(dir, game_from_key(&key.1), &key.2);
        let payload = vec![group.clone()];
        if encode_collisions(&payload)
            .and_then(|raw| std::fs::write(&path, raw).ok())
            .is_none()
        {
            store.dirty.insert(key);
        }
    }
}

fn game_from_key(game: &str) -> Game {
    game.parse().unwrap_or(Game::Skyrim)
}

fn encode_collisions(groups: &[ScriptCollisions]) -> Option<Vec<u8>> {
    let payload = bincode::serialize(groups).ok()?;
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&payload);
    Some(out)
}

fn decode_collisions(raw: &[u8]) -> Result<Vec<ScriptCollisions>, ()> {
    if raw.len() < 8 || raw[..4] != *MAGIC {
        return Err(());
    }
    let version = u32::from_le_bytes(raw[4..8].try_into().map_err(|_| ())?);
    if version != FORMAT_VERSION {
        return Err(());
    }
    bincode::deserialize(&raw[8..]).map_err(|_| ())
}

fn sha256_hex(content: &str) -> String {
    sha256_bytes(content.as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_psc_source(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(source) => source,
        Err(err) => {
            let (source, _encoding, _had_errors) = encoding_rs::WINDOWS_1252.decode(err.as_bytes());
            source.into_owned()
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

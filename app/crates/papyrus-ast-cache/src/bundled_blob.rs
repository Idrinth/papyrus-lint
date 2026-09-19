//! Binary layout of the bundled Skyrim AST/token cache blob.
//!
//! Shared by `build.rs` (writer) and [`crate::bundled`] (reader) via a
//! `#[path]` include from the build script, so this file must stay free of
//! `crate::` / `super::` paths. Entries are keyed by the MD5 of decoded
//! source text — the same digest the on-disk cache stores as `content_md5`
//! — so a vanilla script hits regardless of where it was extracted. Each
//! entry also carries the script's lowercased `ScriptName`, so a type
//! like `Actor` can be resolved when no matching `.psc` is on disk.
//!
//! Each compilation unit only uses half of the API (the build script
//! writes, the library reads), so unused-item warnings are expected.

#![allow(dead_code)]

use std::collections::HashMap;

use papyrus_parser::ast::Script;
use papyrus_parser::token::Token;

/// Four-byte magic at the start of a decompressed blob.
pub const MAGIC: &[u8; 4] = b"PLAC";
/// Layout version. Bump when the header/index/payload encoding changes;
/// the matching `build.rs` rewrites the blob, so a running binary never
/// sees an older layout of its own include.
///
/// Version 2 adds a length-prefixed lowercased script name to each index
/// entry so vanilla types can be looked up by `ScriptName` as well as MD5.
pub const FORMAT_VERSION: u32 = 2;

/// Fixed bytes per index entry, excluding the variable-length name.
const INDEX_ENTRY_FIXED_SIZE: usize = 16 + 2 + 8 + 4 + 8 + 4;

/// One script's location inside the payload section.
#[derive(Clone, Copy)]
pub struct IndexEntry {
    pub ast_offset: u64,
    pub ast_len: u32,
    pub tokens_offset: u64,
    pub tokens_len: u32,
}

/// One compiled script waiting to be packed: MD5 of its decoded source,
/// lowercased `ScriptName`, plus bincode of its AST and token stream.
pub struct PackedEntry {
    pub md5: [u8; 16],
    pub name: String,
    pub ast: Vec<u8>,
    pub tokens: Vec<u8>,
}

/// Serializes `ast` with the same bincode config the reader uses.
pub fn serialize_ast(ast: &Script) -> Option<Vec<u8>> {
    bincode::serialize(ast).ok()
}

/// Serializes `tokens` with the same bincode config the reader uses.
pub fn serialize_tokens(tokens: &[Token]) -> Option<Vec<u8>> {
    bincode::serialize(tokens).ok()
}

/// Packs `entries` into the uncompressed on-disk/in-memory blob layout:
/// magic, format version, count, then a variable-size index (MD5 + name +
/// payload offsets), then the concatenated bincode payloads the index
/// points into. Duplicate names are allowed in the index; the reader keeps
/// the last one, matching zip processing order (SKSE after vanilla).
pub fn encode_blob(entries: &[PackedEntry]) -> Vec<u8> {
    let count = entries.len() as u32;
    let mut payload = Vec::new();
    let mut index = Vec::with_capacity(entries.len());
    for entry in entries {
        let ast_offset = payload.len() as u64;
        payload.extend_from_slice(&entry.ast);
        let tokens_offset = payload.len() as u64;
        payload.extend_from_slice(&entry.tokens);
        index.push((
            entry.md5,
            entry.name.as_bytes(),
            IndexEntry {
                ast_offset,
                ast_len: entry.ast.len() as u32,
                tokens_offset,
                tokens_len: entry.tokens.len() as u32,
            },
        ));
    }

    let index_bytes: usize = index
        .iter()
        .map(|(_, name, _)| INDEX_ENTRY_FIXED_SIZE + name.len())
        .sum();
    let mut out = Vec::with_capacity(12 + index_bytes + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    for (md5, name, loc) in &index {
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
        let name = &name[..name_len as usize];
        out.extend_from_slice(md5);
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&loc.ast_offset.to_le_bytes());
        out.extend_from_slice(&loc.ast_len.to_le_bytes());
        out.extend_from_slice(&loc.tokens_offset.to_le_bytes());
        out.extend_from_slice(&loc.tokens_len.to_le_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

/// Result of [`parse_blob`]: MD5 and name indexes plus the payload offset.
pub struct ParsedBlob {
    pub by_md5: HashMap<[u8; 16], IndexEntry>,
    pub by_name: HashMap<String, IndexEntry>,
    pub payload_start: usize,
}

/// Parses the uncompressed blob into an MD5-keyed index, a lowercased
/// script-name index (last write wins on a duplicate name), and the byte
/// offset where the payload section starts. Returns `None` on a truncated
/// or unrecognized blob; callers treat that as "no bundled cache".
pub fn parse_blob(blob: &[u8]) -> Option<ParsedBlob> {
    if blob.len() < 12 || &blob[..4] != MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(blob[4..8].try_into().ok()?);
    if version != FORMAT_VERSION {
        return None;
    }
    let count = u32::from_le_bytes(blob[8..12].try_into().ok()?) as usize;
    let mut index = HashMap::with_capacity(count);
    let mut by_name = HashMap::with_capacity(count);
    let mut cursor = 12usize;
    for _ in 0..count {
        if blob.len() < cursor.checked_add(INDEX_ENTRY_FIXED_SIZE)? {
            return None;
        }
        let md5: [u8; 16] = blob[cursor..cursor + 16].try_into().ok()?;
        cursor += 16;
        let name_len = u16::from_le_bytes(blob[cursor..cursor + 2].try_into().ok()?) as usize;
        cursor += 2;
        let name_end = cursor.checked_add(name_len)?;
        if blob.len() < name_end.checked_add(8 + 4 + 8 + 4)? {
            return None;
        }
        let name = std::str::from_utf8(&blob[cursor..name_end])
            .ok()?
            .to_string();
        cursor = name_end;
        let ast_offset = u64::from_le_bytes(blob[cursor..cursor + 8].try_into().ok()?);
        cursor += 8;
        let ast_len = u32::from_le_bytes(blob[cursor..cursor + 4].try_into().ok()?);
        cursor += 4;
        let tokens_offset = u64::from_le_bytes(blob[cursor..cursor + 8].try_into().ok()?);
        cursor += 8;
        let tokens_len = u32::from_le_bytes(blob[cursor..cursor + 4].try_into().ok()?);
        cursor += 4;
        let loc = IndexEntry {
            ast_offset,
            ast_len,
            tokens_offset,
            tokens_len,
        };
        index.insert(md5, loc);
        if !name.is_empty() {
            by_name.insert(name, loc);
        }
    }
    Some(ParsedBlob {
        by_md5: index,
        by_name,
        payload_start: cursor,
    })
}

fn payload_slice(payload: &[u8], offset: u64, len: u32) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(len as usize)?;
    payload.get(start..end)
}

/// Deserializes the AST for `entry` from `payload`.
pub fn decode_ast(payload: &[u8], entry: &IndexEntry) -> Option<Script> {
    let bytes = payload_slice(payload, entry.ast_offset, entry.ast_len)?;
    bincode::deserialize(bytes).ok()
}

/// Deserializes the token stream for `entry` from `payload`.
pub fn decode_tokens(payload: &[u8], entry: &IndexEntry) -> Option<Vec<Token>> {
    let bytes = payload_slice(payload, entry.tokens_offset, entry.tokens_len)?;
    bincode::deserialize(bytes).ok()
}

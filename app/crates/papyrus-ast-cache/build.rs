//! Compiles `shared/scripts/skyrim-scripts.zip` and
//! `shared/scripts/skyrim-extender-scripts.zip` into a gzip-compressed AST/token
//! blob (`skyrim-ast-cache.bin.gz` in `OUT_DIR`) that [`bundled`] embeds
//! at compile time. Keyed by MD5 of decoded source *and* by lowercased
//! `ScriptName`, so a known script hits regardless of extract path, and a
//! vanilla type can resolve when no matching `.psc` is on disk. Scripts the
//! parser cannot currently lex are skipped (a `cargo:warning`); an empty
//! blob is a hard error.

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Deserialize;

#[path = "src/bundled_blob.rs"]
mod bundled_blob;
#[path = "src/psc_decode.rs"]
mod psc_decode;

#[derive(Deserialize)]
struct DeprecatedFunction {
    script: String,
    function: String,
}

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let deprecated_path =
        Path::new(&manifest_dir).join("../../../shared/rules/data/deprecated-functions.yaml");
    println!("cargo:rerun-if-changed={}", deprecated_path.display());
    let deprecated: Vec<DeprecatedFunction> =
        serde_norway::from_reader(File::open(&deprecated_path).unwrap_or_else(|err| {
            panic!(
                "failed to open deprecated-function data at {}: {err}",
                deprecated_path.display()
            )
        }))
        .unwrap_or_else(|err| {
            panic!(
                "failed to parse deprecated-function data at {}: {err}",
                deprecated_path.display()
            )
        });
    let started = Instant::now();
    // Insertion order is zip order (vanilla, then SKSE). The name index
    // last-write-wins, so an SKSE script of the same `ScriptName` as a
    // vanilla one is what a no-file lookup resolves. Duplicate digests
    // (identical decoded source) share one AST/token stream; the later
    // zip entry replaces the earlier in place so order stays stable.
    let mut packed: Vec<bundled_blob::PackedEntry> = Vec::new();
    let mut by_md5: HashMap<[u8; 16], usize> = HashMap::new();
    let mut skipped = 0u32;
    for archive_name in ["skyrim-scripts.zip", "skyrim-extender-scripts.zip"] {
        let zip_path = Path::new(&manifest_dir)
            .join("../../../shared/scripts")
            .join(archive_name);
        println!("cargo:rerun-if-changed={}", zip_path.display());
        let file = File::open(&zip_path).unwrap_or_else(|err| {
            panic!(
                "failed to open bundled Skyrim scripts at {}: {err}",
                zip_path.display()
            )
        });
        let mut archive = zip::ZipArchive::new(file).unwrap_or_else(|err| {
            panic!(
                "failed to read bundled Skyrim scripts zip at {}: {err}",
                zip_path.display()
            )
        });
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap_or_else(|err| {
                panic!(
                    "failed to read zip entry {i} from {}: {err}",
                    zip_path.display()
                )
            });
            let name = entry.name().to_string();
            if !name.to_ascii_lowercase().ends_with(".psc") {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap_or_else(|err| {
                panic!("failed to read {name} from {}: {err}", zip_path.display())
            });
            let source = psc_decode::decode_psc_bytes(&bytes);
            let (Ok(mut ast), Ok(tokens)) = (
                papyrus_parser::parse(&source),
                papyrus_parser::tokenize(&source),
            ) else {
                skipped += 1;
                println!("cargo:warning=skipping {name}: papyrus-parser could not lex/parse it");
                continue;
            };
            mark_deprecated_functions(&mut ast, &deprecated);
            let Some(ast_bytes) = bundled_blob::serialize_ast(&ast) else {
                skipped += 1;
                println!("cargo:warning=skipping {name}: AST failed to serialize");
                continue;
            };
            let Some(token_bytes) = bundled_blob::serialize_tokens(&tokens) else {
                skipped += 1;
                println!("cargo:warning=skipping {name}: tokens failed to serialize");
                continue;
            };
            let md5 = md5::compute(source.as_bytes()).0;
            let packed_entry = bundled_blob::PackedEntry {
                md5,
                name: ast.name.to_ascii_lowercase(),
                ast: ast_bytes,
                tokens: token_bytes,
            };
            if let Some(&idx) = by_md5.get(&md5) {
                packed[idx] = packed_entry;
            } else {
                by_md5.insert(md5, packed.len());
                packed.push(packed_entry);
            }
        }
    }

    if packed.is_empty() {
        panic!("bundled Skyrim AST cache is empty — papyrus-parser produced no usable scripts");
    }

    let count = packed.len();
    let raw = bundled_blob::encode_blob(&packed);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&raw)
        .unwrap_or_else(|err| panic!("failed to gzip bundled Skyrim AST cache: {err}"));
    let compressed = encoder
        .finish()
        .unwrap_or_else(|err| panic!("failed to finish gzip of bundled Skyrim AST cache: {err}"));

    let dest = Path::new(&out_dir).join("skyrim-ast-cache.bin.gz");
    std::fs::write(&dest, compressed).unwrap_or_else(|err| {
        panic!(
            "failed to write bundled Skyrim AST cache to {}: {err}",
            dest.display()
        )
    });

    println!(
        "cargo:warning=bundled Skyrim AST cache: {count} scripts ({skipped} skipped) in {:?}",
        started.elapsed()
    );
}

fn mark_deprecated_functions(
    ast: &mut papyrus_parser::ast::Script,
    deprecated: &[DeprecatedFunction],
) {
    for rule in deprecated
        .iter()
        .filter(|rule| rule.script.eq_ignore_ascii_case(&ast.name))
    {
        for function in ast
            .functions
            .iter_mut()
            .chain(ast.states.iter_mut().flat_map(|state| &mut state.functions))
        {
            if function.name.eq_ignore_ascii_case(&rule.function) {
                function.deprecated = true;
            }
        }
    }
}

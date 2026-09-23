//! Compiles each supported game's vanilla/extender script archives under
//! `shared/scripts/` (Skyrim: `skyrim-scripts.zip` +
//! `skyrim-extender-scripts.zip`; Fallout 4: `fallout4-scripts.zip` +
//! `fallout4-extender-scripts.zip`) into a gzip-compressed AST/token blob
//! per game (`{game}-ast-cache.bin.gz` in `OUT_DIR`) that [`bundled`]
//! embeds at compile time. Keyed by MD5 of decoded source *and* by
//! lowercased `ScriptName`, so a known script hits regardless of extract
//! path, and a vanilla type can resolve when no matching `.psc` is on
//! disk. Scripts the parser cannot currently lex are skipped (a
//! `cargo:warning`); an empty blob is a hard error.

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use flate2::write::GzEncoder;
use flate2::Compression;
use papyrus_parser::parser::GameEdition;
use serde::Deserialize;

#[path = "src/bundled_blob.rs"]
mod bundled_blob;
#[path = "src/psc_decode.rs"]
mod psc_decode;

#[derive(Deserialize)]
struct DeprecatedFunction {
    script: String,
    function: String,
    replacement: Option<String>,
    message: String,
}

/// One supported game's bundled-blob build inputs: its `shared/scripts`
/// archives, its `deprecated-functions.yaml`, the parser dialect its
/// scripts parse under, and the `OUT_DIR` filename [`bundled`] embeds.
struct GameArchives {
    display_name: &'static str,
    archives: [&'static str; 2],
    deprecated_data: &'static str,
    mode: GameEdition,
    out_file: &'static str,
}

const GAMES: [GameArchives; 2] = [
    GameArchives {
        display_name: "Skyrim",
        archives: ["skyrim-scripts.zip", "skyrim-extender-scripts.zip"],
        deprecated_data: "skyrim/deprecated-functions.yaml",
        mode: GameEdition::Skyrim,
        out_file: "skyrim-ast-cache.bin.gz",
    },
    GameArchives {
        display_name: "Fallout 4",
        archives: ["fallout4-scripts.zip", "fallout4-extender-scripts.zip"],
        deprecated_data: "fallout4/deprecated-functions.yaml",
        mode: GameEdition::Fallout4,
        out_file: "fallout4-ast-cache.bin.gz",
    },
];

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");

    for game in &GAMES {
        build_game_blob(&manifest_dir, &out_dir, game);
    }
}

fn build_game_blob(manifest_dir: &str, out_dir: &str, game: &GameArchives) {
    let deprecated_path = Path::new(manifest_dir)
        .join("../../../shared/rules/data")
        .join(game.deprecated_data);
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
    // Insertion order is zip order (vanilla, then extender). The name index
    // last-write-wins, so an extender script of the same `ScriptName` as a
    // vanilla one is what a no-file lookup resolves. Duplicate digests
    // (identical decoded source) share one AST/token stream; the later
    // zip entry replaces the earlier in place so order stays stable.
    let mut packed: Vec<bundled_blob::PackedEntry> = Vec::new();
    let mut by_md5: HashMap<[u8; 16], usize> = HashMap::new();
    let mut skipped = 0u32;
    for archive_name in game.archives {
        let zip_path = Path::new(manifest_dir)
            .join("../../../shared/scripts")
            .join(archive_name);
        println!("cargo:rerun-if-changed={}", zip_path.display());
        let file = File::open(&zip_path).unwrap_or_else(|err| {
            panic!(
                "failed to open bundled {} scripts at {}: {err}",
                game.display_name,
                zip_path.display()
            )
        });
        let mut archive = zip::ZipArchive::new(file).unwrap_or_else(|err| {
            panic!(
                "failed to read bundled {} scripts zip at {}: {err}",
                game.display_name,
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
                papyrus_parser::parse_with_mode(&source, game.mode),
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
        panic!(
            "bundled {} AST cache is empty — papyrus-parser produced no usable scripts",
            game.display_name
        );
    }

    let count = packed.len();
    let raw = bundled_blob::encode_blob(&packed);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).unwrap_or_else(|err| {
        panic!(
            "failed to gzip bundled {} AST cache: {err}",
            game.display_name
        )
    });
    let compressed = encoder.finish().unwrap_or_else(|err| {
        panic!(
            "failed to finish gzip of bundled {} AST cache: {err}",
            game.display_name
        )
    });

    let dest = Path::new(out_dir).join(game.out_file);
    std::fs::write(&dest, compressed).unwrap_or_else(|err| {
        panic!(
            "failed to write bundled {} AST cache to {}: {err}",
            game.display_name,
            dest.display()
        )
    });

    println!(
        "cargo:warning=bundled {} AST cache: {count} scripts ({skipped} skipped) in {:?}",
        game.display_name,
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
                function.deprecation = Some(papyrus_parser::ast::Deprecation {
                    replacement: rule.replacement.clone(),
                    message: format!("{}.{}: {}", rule.script, rule.function, rule.message),
                });
            }
        }
    }
}

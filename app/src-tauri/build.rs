//! Generates the Tauri icon set under `icons/` from
//! `resources/logo.png` so those platform-specific variants are not
//! checked in.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageEncoder, RgbaImage};

fn main() {
    generate_icons();
    tauri_build::build();
}

fn generate_icons() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let source = manifest_dir.join("../../resources/logo.png");
    println!("cargo:rerun-if-changed={}", source.display());

    let icons_dir = manifest_dir.join("icons");
    fs::create_dir_all(&icons_dir).expect("create src-tauri/icons");

    let outputs = icon_outputs(&icons_dir);
    if outputs_are_fresh(&source, &outputs) {
        return;
    }

    let original = image::open(&source)
        .unwrap_or_else(|err| panic!("failed to open {}: {err}", source.display()))
        .into_rgba8();

    let mut png_by_size: HashMap<u32, Vec<u8>> = HashMap::new();
    let mut png = |size: u32| -> Vec<u8> {
        png_by_size
            .entry(size)
            .or_insert_with(|| encode_png(&resize(&original, size)))
            .clone()
    };

    write_bytes(&icons_dir.join("32x32.png"), &png(32));
    write_bytes(&icons_dir.join("128x128.png"), &png(128));
    write_bytes(&icons_dir.join("128x128@2x.png"), &png(256));
    write_bytes(&icons_dir.join("icon.png"), &png(512));

    for (name, size) in [
        ("Square30x30Logo.png", 30),
        ("Square44x44Logo.png", 44),
        ("Square71x71Logo.png", 71),
        ("Square89x89Logo.png", 89),
        ("Square107x107Logo.png", 107),
        ("Square142x142Logo.png", 142),
        ("Square150x150Logo.png", 150),
        ("Square284x284Logo.png", 284),
        ("Square310x310Logo.png", 310),
        ("StoreLogo.png", 50),
    ] {
        write_bytes(&icons_dir.join(name), &png(size));
    }

    let ico_frames: Vec<(u32, Vec<u8>)> = [32_u32, 16, 24, 48, 64, 256]
        .into_iter()
        .map(|size| (size, png(size)))
        .collect();
    write_ico(&icons_dir.join("icon.ico"), &ico_frames);

    let icns_entries: Vec<([u8; 4], Vec<u8>)> = [
        (*b"icp4", 16_u32),
        (*b"icp5", 32),
        (*b"icp6", 64),
        (*b"ic07", 128),
        (*b"ic08", 256),
        (*b"ic09", 512),
        (*b"ic10", 1024),
    ]
    .into_iter()
    .map(|(ostype, size)| (ostype, png(size)))
    .collect();
    write_icns(&icons_dir.join("icon.icns"), &icns_entries);
}

fn icon_outputs(icons_dir: &Path) -> Vec<PathBuf> {
    [
        "32x32.png",
        "128x128.png",
        "128x128@2x.png",
        "icon.png",
        "icon.ico",
        "icon.icns",
        "Square30x30Logo.png",
        "Square44x44Logo.png",
        "Square71x71Logo.png",
        "Square89x89Logo.png",
        "Square107x107Logo.png",
        "Square142x142Logo.png",
        "Square150x150Logo.png",
        "Square284x284Logo.png",
        "Square310x310Logo.png",
        "StoreLogo.png",
    ]
    .into_iter()
    .map(|name| icons_dir.join(name))
    .collect()
}

fn outputs_are_fresh(source: &Path, outputs: &[PathBuf]) -> bool {
    let Ok(src_mtime) = fs::metadata(source).and_then(|meta| meta.modified()) else {
        return false;
    };
    outputs.iter().all(|path| {
        fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .is_some_and(|mtime| mtime >= src_mtime)
    })
}

fn resize(source: &RgbaImage, size: u32) -> RgbaImage {
    imageops::resize(source, size, size, FilterType::Lanczos3)
}

fn encode_png(image: &RgbaImage) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    PngEncoder::new_with_quality(&mut buf, CompressionType::Best, PngFilterType::Adaptive)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("encode PNG");
    buf.into_inner()
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
}

fn write_ico(path: &Path, frames: &[(u32, Vec<u8>)]) {
    let mut data = Vec::new();
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * frames.len() as u32;
    for (size, png) in frames {
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        data.push(dim);
        data.push(dim);
        data.push(0);
        data.push(0);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data.extend_from_slice(&(png.len() as u32).to_le_bytes());
        data.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in frames {
        data.extend_from_slice(png);
    }
    write_bytes(path, &data);
}

fn write_icns(path: &Path, entries: &[([u8; 4], Vec<u8>)]) {
    let body_len: usize = entries.iter().map(|(_, png)| 8 + png.len()).sum();
    let mut data = Vec::with_capacity(8 + body_len);
    data.extend_from_slice(b"icns");
    data.extend_from_slice(&((8 + body_len) as u32).to_be_bytes());
    for (ostype, png) in entries {
        data.extend_from_slice(ostype);
        data.extend_from_slice(&((8 + png.len()) as u32).to_be_bytes());
        data.extend_from_slice(png);
    }
    write_bytes(path, &data);
}

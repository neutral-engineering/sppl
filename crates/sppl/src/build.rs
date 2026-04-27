//! Helpers intended to be called from a downstream `build.rs`.
//!
//! ```no_run
//! // build.rs
//! fn main() {
//!     // ... whatever produces your Svelte build directory ...
//!     sppl::build::gzip_assets("../app/build").unwrap();
//! }
//! ```
//!
//! `gzip_assets` walks the directory, replacing every compressible file
//! `<name>.<ext>` with `<name>.<ext>.gz`. At runtime, [`crate::resolve`]
//! prefers the `.gz` variant; the axum handler sends it as-is to clients
//! that accept gzip and decompresses on the fly otherwise.

use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;

/// File extensions whose contents typically compress well. Binary formats
/// that are already compressed (PNG/JPEG/WOFF2/MP4/…) are skipped to avoid
/// spending bytes for no gain.
pub const DEFAULT_COMPRESSIBLE_EXTENSIONS: &[&str] = &[
    "html", "htm", "css", "js", "mjs", "cjs", "json", "map", "svg", "wasm", "txt", "xml", "ico",
    "webmanifest",
];

/// Gzip every file in `dir` (recursively) whose extension is in
/// [`DEFAULT_COMPRESSIBLE_EXTENSIONS`], replacing it with a `.gz` sibling.
pub fn gzip_assets<P: AsRef<Path>>(dir: P) -> std::io::Result<()> {
    gzip_assets_with(dir, DEFAULT_COMPRESSIBLE_EXTENSIONS)
}

/// Like [`gzip_assets`] but with a caller-provided extension allow-list.
pub fn gzip_assets_with<P: AsRef<Path>>(dir: P, extensions: &[&str]) -> std::io::Result<()> {
    walk(dir.as_ref(), extensions)
}

fn walk(dir: &Path, exts: &[&str]) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk(&path, exts)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if ext == "gz" || !exts.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }

        let bytes = fs::read(&path)?;
        let gz_path = {
            let mut p = path.clone().into_os_string();
            p.push(".gz");
            std::path::PathBuf::from(p)
        };
        let f = fs::File::create(&gz_path)?;
        let mut enc = GzEncoder::new(f, Compression::best());
        enc.write_all(&bytes)?;
        enc.finish()?;
        fs::remove_file(&path)?;
    }
    Ok(())
}

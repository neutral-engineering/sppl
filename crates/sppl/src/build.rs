//! Helpers intended to be called from a downstream `build.rs`.
//!
//! ```no_run
//! // build.rs
//! fn main() {
//!     // ... whatever produces your Svelte build directory ...
//!     sppl::build::compress_assets("../app/build", &[
//!         sppl::build::Algorithm::Brotli,
//!         sppl::build::Algorithm::Gzip,
//!     ]).unwrap();
//! }
//! ```
//!
//! `compress_assets` walks the directory, leaving the original file in place
//! and writing one sibling per algorithm (`<name>.<ext>.br`,
//! `<name>.<ext>.gz`). At runtime [`crate::resolve`] prefers `.br`, then
//! `.gz`, then the raw file; the axum handler picks whichever the client
//! accepts.
//!
//! For backwards compatibility [`gzip_assets`] is kept: it produces only the
//! `.gz` variant and *removes* the source file, matching pre-brotli behavior.

use std::fs;
use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;

/// File extensions whose contents typically compress well. Binary formats
/// that are already compressed (PNG/JPEG/WOFF2/MP4/…) are skipped to avoid
/// spending bytes for no gain.
pub const DEFAULT_COMPRESSIBLE_EXTENSIONS: &[&str] = &[
    "html",
    "htm",
    "css",
    "js",
    "mjs",
    "cjs",
    "json",
    "map",
    "svg",
    "wasm",
    "txt",
    "xml",
    "ico",
    "webmanifest",
];

/// Compression algorithm to apply to compressible assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    /// gzip (`Content-Encoding: gzip`). Suffix: `.gz`.
    Gzip,
    /// Brotli (`Content-Encoding: br`). Suffix: `.br`. Typically 15–25%
    /// smaller than gzip on HTML/JS/CSS.
    Brotli,
}

impl Algorithm {
    fn suffix(self) -> &'static str {
        match self {
            Algorithm::Gzip => "gz",
            Algorithm::Brotli => "br",
        }
    }

    fn encode(self, src: &[u8], dst: &mut Vec<u8>) -> std::io::Result<()> {
        match self {
            Algorithm::Gzip => {
                let mut enc = GzEncoder::new(dst, Compression::best());
                enc.write_all(src)?;
                enc.finish()?;
                Ok(())
            }
            Algorithm::Brotli => {
                // quality 11 = best, lgwin 22 = 4 MiB sliding window
                // (brotli's default for "max compression").
                let mut input = src;
                brotli::BrotliCompress(
                    &mut input,
                    dst,
                    &brotli::enc::BrotliEncoderParams {
                        quality: 11,
                        lgwin: 22,
                        ..Default::default()
                    },
                )?;
                Ok(())
            }
        }
    }
}

/// Pre-compress every file in `dir` (recursively) whose extension is in
/// [`DEFAULT_COMPRESSIBLE_EXTENSIONS`], producing one sibling per requested
/// algorithm and *leaving the original in place*. This is the recommended
/// helper for new code — clients that don't accept any compression still
/// get a working asset.
pub fn compress_assets<P: AsRef<Path>>(
    dir: P,
    algorithms: &[Algorithm],
) -> std::io::Result<()> {
    compress_assets_with(dir, algorithms, DEFAULT_COMPRESSIBLE_EXTENSIONS)
}

/// Like [`compress_assets`] but with a caller-provided extension allow-list.
pub fn compress_assets_with<P: AsRef<Path>>(
    dir: P,
    algorithms: &[Algorithm],
    extensions: &[&str],
) -> std::io::Result<()> {
    walk(dir.as_ref(), algorithms, extensions, /* drop_source = */ false)
}

/// Gzip every file in `dir` (recursively), replacing each with a `.gz`
/// sibling. Kept for backwards compatibility; new code should prefer
/// [`compress_assets`], which produces both `.br` and `.gz` variants and
/// retains the originals.
pub fn gzip_assets<P: AsRef<Path>>(dir: P) -> std::io::Result<()> {
    gzip_assets_with(dir, DEFAULT_COMPRESSIBLE_EXTENSIONS)
}

/// Like [`gzip_assets`] but with a caller-provided extension allow-list.
pub fn gzip_assets_with<P: AsRef<Path>>(dir: P, extensions: &[&str]) -> std::io::Result<()> {
    walk(dir.as_ref(), &[Algorithm::Gzip], extensions, /* drop_source = */ true)
}

fn walk(
    dir: &Path,
    algorithms: &[Algorithm],
    exts: &[&str],
    drop_source: bool,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            walk(&path, algorithms, exts, drop_source)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        // Skip already-compressed siblings produced by a previous run.
        if ext == "gz" || ext == "br" {
            continue;
        }
        if !exts.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }

        let bytes = fs::read(&path)?;
        for &alg in algorithms {
            let mut out = Vec::with_capacity(bytes.len());
            alg.encode(&bytes, &mut out)?;
            let dst_path = {
                let mut p = path.clone().into_os_string();
                p.push(".");
                p.push(alg.suffix());
                std::path::PathBuf::from(p)
            };
            fs::write(&dst_path, &out)?;
        }
        if drop_source {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

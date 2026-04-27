//! sppl — embed static Svelte (and other static) apps into Rust servers.
//!
//! ```ignore
//! #[derive(sppl::RustEmbed)]
//! #[folder = "$CARGO_MANIFEST_DIR/../app/build"]
//! #[crate_path = "sppl::rust_embed"]
//! struct App;
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = sppl::axum::router::<App>();
//!     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
//! }
//! ```
//!
//! ## Compression
//!
//! `sppl` understands a directory in which compressible files have been
//! pre-gzipped (e.g. `index.html` → `index.html.gz`). Use
//! [`build::gzip_assets`] from your `build.rs` to do it once at compile time;
//! at request time `sppl::resolve` prefers the `.gz` variant if present.
//!
//! - clients that send `Accept-Encoding: gzip` get the raw bytes from the
//!   binary with `Content-Encoding: gzip` (no per-request CPU cost),
//! - clients that don't are served decompressed bytes via [`flate2`].

use std::borrow::Cow;

pub use rust_embed::{EmbeddedFile, RustEmbed};

// Re-export the whole `rust_embed` crate so the `RustEmbed` derive can find
// its support items via `#[crate_path = "sppl::rust_embed"]` without users
// having to add `rust-embed` to their own `Cargo.toml`.
#[doc(hidden)]
pub use rust_embed;

#[cfg(feature = "axum")]
pub mod axum;

pub mod build;

/// A resolved embedded asset, after applying SvelteKit `adapter-static` and
/// SPA-fallback lookup rules and after picking a `.gz` variant if available.
pub struct Asset {
    /// The matched logical path (without any `.gz` suffix), useful for
    /// deriving a `Content-Type`.
    pub path: String,
    /// The bytes as stored in the binary. `gzipped` says whether they are
    /// gzip-compressed.
    pub data: Cow<'static, [u8]>,
    /// `true` if `data` is gzip-encoded.
    pub gzipped: bool,
}

impl Asset {
    fn from_embedded(path: String, file: EmbeddedFile, gzipped: bool) -> Self {
        Self {
            path,
            data: file.data,
            gzipped,
        }
    }

    /// Returns the bytes you'd send to a client that does NOT accept gzip:
    /// either the raw bytes (if not gzipped) or freshly decompressed bytes.
    pub fn decoded(&self) -> std::io::Result<Cow<'_, [u8]>> {
        if !self.gzipped {
            return Ok(Cow::Borrowed(&self.data));
        }
        use std::io::Read;
        let mut out = Vec::with_capacity(self.data.len() * 4);
        flate2::read::GzDecoder::new(&self.data[..]).read_to_end(&mut out)?;
        Ok(Cow::Owned(out))
    }
}

/// Look up a file in an embedded asset bundle, applying the resolution rules
/// that match SvelteKit's `adapter-static` output, with a `.gz` fast-path:
///
/// For each candidate path we first try `<candidate>.gz`, then `<candidate>`:
///
/// 1. exact path
/// 2. `<path>.html`            (prerendered route)
/// 3. `<path>/index.html`      (prerendered route with trailing slash)
/// 4. `index.html`             (SPA fallback)
pub fn resolve<A: RustEmbed>(path: &str) -> Option<Asset> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    let candidates: [String; 4] = [
        path.to_string(),
        format!("{}.html", path.trim_end_matches('/')),
        format!("{}/index.html", path.trim_end_matches('/')),
        "index.html".to_string(),
    ];

    for candidate in candidates {
        let gz = format!("{candidate}.gz");
        if let Some(file) = A::get(&gz) {
            return Some(Asset::from_embedded(candidate, file, true));
        }
        if let Some(file) = A::get(&candidate) {
            return Some(Asset::from_embedded(candidate, file, false));
        }
    }
    None
}

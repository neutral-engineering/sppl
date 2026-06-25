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
//! pre-compressed (`<name>.<ext>.br` for brotli, `<name>.<ext>.gz` for gzip).
//! Use the helpers in [`build`] from your `build.rs` to do it once at compile
//! time; at request time [`resolve`] prefers `.br`, then `.gz`, then the raw
//! file.
//!
//! By default the axum router picks the best variant the client accepts and
//! sends the stored bytes as-is. Clients that don't advertise `br` still get
//! the gzip variant (every modern HTTP library decompresses gzip
//! transparently); flip [`axum::RouterConfig::never_decompress`] to `false`
//! to opt back into on-the-fly decompression for clients that genuinely
//! can't accept gzip.

#![allow(clippy::needless_doctest_main)]

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

/// How an [`Asset`]'s bytes are encoded on disk / in the binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// Raw bytes — no content encoding.
    Identity,
    /// Gzip-compressed (`Content-Encoding: gzip`).
    Gzip,
    /// Brotli-compressed (`Content-Encoding: br`).
    Brotli,
}

impl Encoding {
    /// The HTTP `Content-Encoding` token, or `None` for [`Encoding::Identity`].
    pub fn content_encoding(self) -> Option<&'static str> {
        match self {
            Encoding::Identity => None,
            Encoding::Gzip => Some("gzip"),
            Encoding::Brotli => Some("br"),
        }
    }

    /// Filename suffix (without the leading dot), or `None` for identity.
    pub fn suffix(self) -> Option<&'static str> {
        match self {
            Encoding::Identity => None,
            Encoding::Gzip => Some("gz"),
            Encoding::Brotli => Some("br"),
        }
    }
}

/// Default lookup order: brotli, then gzip, then the raw file.
pub const DEFAULT_ENCODINGS: &[Encoding] =
    &[Encoding::Brotli, Encoding::Gzip, Encoding::Identity];

/// A resolved embedded asset, after applying SvelteKit `adapter-static` and
/// SPA-fallback lookup rules and after picking a pre-compressed variant if
/// available.
pub struct Asset {
    /// The matched logical path (without any compression suffix), useful for
    /// deriving a `Content-Type`.
    pub path: String,
    /// The bytes as stored in the binary. `encoding` says how they're encoded.
    pub data: Cow<'static, [u8]>,
    /// How `data` is encoded.
    pub encoding: Encoding,
}

impl Asset {
    fn from_embedded(path: String, file: EmbeddedFile, encoding: Encoding) -> Self {
        Self {
            path,
            data: file.data,
            encoding,
        }
    }

    /// Returns the bytes you'd send to a client that doesn't accept any
    /// content encoding — i.e. the raw bytes, decompressed if needed.
    pub fn decoded(&self) -> std::io::Result<Cow<'_, [u8]>> {
        match self.encoding {
            Encoding::Identity => Ok(Cow::Borrowed(&self.data)),
            Encoding::Gzip => {
                use std::io::Read;
                let mut out = Vec::with_capacity(self.data.len() * 4);
                flate2::read::GzDecoder::new(&self.data[..]).read_to_end(&mut out)?;
                Ok(Cow::Owned(out))
            }
            Encoding::Brotli => {
                use std::io::Read;
                let mut out = Vec::with_capacity(self.data.len() * 4);
                brotli::Decompressor::new(&self.data[..], 4096).read_to_end(&mut out)?;
                Ok(Cow::Owned(out))
            }
        }
    }
}

/// Look up a file in an embedded asset bundle using [`DEFAULT_ENCODINGS`]
/// (brotli, then gzip, then raw). For finer control — e.g. respecting a
/// client's `Accept-Encoding` — use [`resolve_with`].
pub fn resolve<A: RustEmbed>(path: &str) -> Option<Asset> {
    resolve_with::<A>(path, DEFAULT_ENCODINGS)
}

/// Look up a file in an embedded asset bundle, applying the resolution rules
/// that match SvelteKit's `adapter-static` output. For each candidate path
/// we try the given `encodings` in order:
///
/// 1. exact path
/// 2. `<path>.html`            (prerendered route)
/// 3. `<path>/index.html`      (prerendered route with trailing slash)
/// 4. `index.html`             (SPA fallback)
pub fn resolve_with<A: RustEmbed>(path: &str, encodings: &[Encoding]) -> Option<Asset> {
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    let candidates: [String; 4] = [
        path.to_string(),
        format!("{}.html", path.trim_end_matches('/')),
        format!("{}/index.html", path.trim_end_matches('/')),
        "index.html".to_string(),
    ];

    for candidate in candidates {
        for &enc in encodings {
            let lookup = match enc.suffix() {
                Some(suffix) => format!("{candidate}.{suffix}"),
                None => candidate.clone(),
            };
            if let Some(file) = A::get(&lookup) {
                return Some(Asset::from_embedded(candidate, file, enc));
            }
        }
    }
    None
}

//! Axum integration.

use ::axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

use crate::{RustEmbed, resolve};

/// Runtime knobs for [`router_with`].
#[derive(Clone, Debug, Default)]
pub struct RouterConfig {
    /// When `true`, never gunzip on the fly — clients that don't advertise
    /// `Accept-Encoding: gzip` get the gzipped bytes anyway, with
    /// `Content-Encoding: gzip` set. Caps CPU cost under load (e.g. a script
    /// hammering the server with `curl` and no `--compressed`). Practically
    /// every modern client decompresses gzip transparently.
    pub never_decompress: bool,
}

/// Build a [`Router`] that serves the embedded assets of `A` on every path,
/// with SvelteKit `adapter-static` semantics, an SPA fallback to
/// `index.html`, and transparent gzip handling (see crate docs).
///
/// Mount it at the root, or nest it under a prefix:
///
/// ```ignore
/// let api = Router::new().route("/api/hello", get(|| async { "hi" }));
/// let app = api.fallback_service(sppl::axum::router::<App>());
/// ```
pub fn router<A>() -> Router
where
    A: RustEmbed + Send + Sync + 'static,
{
    router_with::<A>(RouterConfig::default())
}

/// Like [`router`], but with overrides. Use this to opt out of on-the-fly
/// decompression (see [`RouterConfig::never_decompress`]).
pub fn router_with<A>(config: RouterConfig) -> Router
where
    A: RustEmbed + Send + Sync + 'static,
{
    Router::new().fallback(handler::<A>).with_state(config)
}

/// What we send back for a given (asset, request, config) triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// Asset isn't gzipped — just send the raw bytes.
    RawAsIs,
    /// Asset is gzipped and the client either advertised gzip or we're
    /// configured to never decompress; send the gzipped bytes with
    /// `Content-Encoding: gzip`.
    GzippedAsIs,
    /// Asset is gzipped and the client doesn't advertise gzip and we are
    /// allowed to decompress on the fly.
    Decompress,
}

fn pick_encoding(asset_gzipped: bool, accepts_gzip: bool, config: &RouterConfig) -> Encoding {
    if !asset_gzipped {
        Encoding::RawAsIs
    } else if accepts_gzip || config.never_decompress {
        Encoding::GzippedAsIs
    } else {
        Encoding::Decompress
    }
}

async fn handler<A: RustEmbed>(
    State(config): State<RouterConfig>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let Some(asset) = resolve::<A>(uri.path()) else {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    };

    let mime = mime_guess::from_path(&asset.path).first_or_octet_stream();
    let encoding = pick_encoding(asset.gzipped, accepts_gzip(&headers), &config);

    match encoding {
        Encoding::GzippedAsIs => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CONTENT_ENCODING, "gzip")
            .header(header::VARY, "Accept-Encoding")
            .body(Body::from(asset.data.into_owned()))
            .unwrap(),
        Encoding::Decompress => match asset.decoded() {
            Ok(decoded) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::VARY, "Accept-Encoding")
                .body(Body::from(decoded.into_owned()))
                .unwrap(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "decompression failed").into_response(),
        },
        Encoding::RawAsIs => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(asset.data.into_owned()))
            .unwrap(),
    }
}

fn accepts_gzip(headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::ACCEPT_ENCODING) else {
        return false;
    };
    let Ok(s) = value.to_str() else {
        return false;
    };
    s.split(',').any(|enc| {
        // Strip optional `;q=...` parameter and surrounding whitespace.
        let token = enc.split(';').next().unwrap_or("").trim();
        token.eq_ignore_ascii_case("gzip")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, value.parse().unwrap());
        headers
    }

    #[test]
    fn missing_header_is_false() {
        assert!(!accepts_gzip(&HeaderMap::new()));
    }

    #[test]
    fn empty_header_is_false() {
        assert!(!accepts_gzip(&h("")));
    }

    #[test]
    fn plain_gzip_is_true() {
        assert!(accepts_gzip(&h("gzip")));
    }

    #[test]
    fn case_insensitive() {
        assert!(accepts_gzip(&h("GZIP")));
        assert!(accepts_gzip(&h("Gzip")));
    }

    #[test]
    fn finds_gzip_anywhere_in_list() {
        assert!(accepts_gzip(&h("gzip, deflate")));
        assert!(accepts_gzip(&h("deflate, gzip")));
        assert!(accepts_gzip(&h("br, gzip, deflate")));
    }

    #[test]
    fn ignores_q_parameter() {
        assert!(accepts_gzip(&h("gzip;q=0.5")));
        assert!(accepts_gzip(&h("gzip; q=0.8")));
        assert!(accepts_gzip(&h("deflate, gzip;q=0.9, br")));
    }

    #[test]
    fn surrounding_whitespace_ok() {
        assert!(accepts_gzip(&h("  gzip  ")));
        assert!(accepts_gzip(&h(" deflate ,  gzip ")));
    }

    #[test]
    fn other_encodings_alone_are_false() {
        assert!(!accepts_gzip(&h("deflate")));
        assert!(!accepts_gzip(&h("br")));
        assert!(!accepts_gzip(&h("identity")));
        assert!(!accepts_gzip(&h("br, deflate, identity")));
    }

    #[test]
    fn substring_match_is_not_enough() {
        // "x-gzip" and "gzipped" are distinct tokens, not gzip.
        assert!(!accepts_gzip(&h("x-gzip")));
        assert!(!accepts_gzip(&h("gzipped")));
    }

    fn default_cfg() -> RouterConfig {
        RouterConfig::default()
    }

    fn never_decompress_cfg() -> RouterConfig {
        RouterConfig {
            never_decompress: true,
        }
    }

    #[test]
    fn raw_assets_are_always_sent_as_is() {
        assert_eq!(
            pick_encoding(false, true, &default_cfg()),
            Encoding::RawAsIs
        );
        assert_eq!(
            pick_encoding(false, false, &default_cfg()),
            Encoding::RawAsIs
        );
        assert_eq!(
            pick_encoding(false, false, &never_decompress_cfg()),
            Encoding::RawAsIs
        );
    }

    #[test]
    fn gzipped_to_gzip_aware_client_is_sent_as_is() {
        assert_eq!(
            pick_encoding(true, true, &default_cfg()),
            Encoding::GzippedAsIs
        );
    }

    #[test]
    fn default_config_decompresses_for_unaware_clients() {
        assert_eq!(
            pick_encoding(true, false, &default_cfg()),
            Encoding::Decompress
        );
    }

    #[test]
    fn never_decompress_overrides_unaware_client() {
        // Even though the client didn't send Accept-Encoding: gzip, we send
        // gzipped bytes anyway — that's the whole point of the flag.
        assert_eq!(
            pick_encoding(true, false, &never_decompress_cfg()),
            Encoding::GzippedAsIs
        );
    }
}

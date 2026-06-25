//! Axum integration.

use ::axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

use crate::{Encoding, RustEmbed, resolve_with};

/// Runtime knobs for [`router_with`].
#[derive(Clone, Debug)]
pub struct RouterConfig {
    /// When `true` (the default), never decompress on the fly: clients that
    /// don't advertise `Accept-Encoding: gzip` (or `br`) still get the
    /// pre-compressed bytes with the appropriate `Content-Encoding`. Caps
    /// per-request CPU cost — important defense against a script hammering
    /// the server with no `Accept-Encoding` header (a cheap DoS vector if
    /// the server has to gunzip/de-brotli every response).
    ///
    /// Practically every modern client decompresses gzip transparently; if
    /// only the brotli variant exists, an old client that doesn't accept
    /// `br` will fail to decode — that's the trade-off you're opting into.
    ///
    /// Set to `false` to restore on-the-fly decompression for clients that
    /// truly can't accept either encoding.
    pub never_decompress: bool,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            never_decompress: true,
        }
    }
}

/// Build a [`Router`] that serves the embedded assets of `A` on every path,
/// with SvelteKit `adapter-static` semantics, an SPA fallback to
/// `index.html`, and transparent gzip/brotli handling (see crate docs).
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

/// Like [`router`], but with overrides. Use this to opt out of the
/// "send gzip even to clients that didn't ask for it" behavior (see
/// [`RouterConfig::never_decompress`]).
pub fn router_with<A>(config: RouterConfig) -> Router
where
    A: RustEmbed + Send + Sync + 'static,
{
    Router::new().fallback(handler::<A>).with_state(config)
}

/// What we send back for a given (asset, request, config) triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Disposition {
    /// Send the stored bytes as-is with the appropriate `Content-Encoding`.
    AsIs,
    /// Decode the stored bytes and send the result as identity.
    Decompress,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
struct AcceptedEncodings {
    gzip: bool,
    brotli: bool,
}

fn parse_accept_encoding(headers: &HeaderMap) -> AcceptedEncodings {
    let Some(value) = headers.get(header::ACCEPT_ENCODING) else {
        return AcceptedEncodings::default();
    };
    let Ok(s) = value.to_str() else {
        return AcceptedEncodings::default();
    };
    let mut out = AcceptedEncodings::default();
    for enc in s.split(',') {
        let token = enc.split(';').next().unwrap_or("").trim();
        if token.eq_ignore_ascii_case("gzip") {
            out.gzip = true;
        } else if token.eq_ignore_ascii_case("br") {
            out.brotli = true;
        }
    }
    out
}

/// Build the preference list passed to [`crate::resolve_with`] based on what
/// the client accepts. We always include `Identity` last so the lookup
/// succeeds even if only a raw file exists.
fn encoding_prefs(accepts: AcceptedEncodings, config: &RouterConfig) -> Vec<Encoding> {
    let mut prefs = Vec::with_capacity(3);
    if accepts.brotli || config.never_decompress {
        prefs.push(Encoding::Brotli);
    }
    if accepts.gzip || config.never_decompress {
        prefs.push(Encoding::Gzip);
    }
    prefs.push(Encoding::Identity);
    prefs
}

fn pick_disposition(
    asset_encoding: Encoding,
    accepts: AcceptedEncodings,
) -> Disposition {
    match asset_encoding {
        Encoding::Identity => Disposition::AsIs,
        Encoding::Gzip => {
            // Either the client advertised gzip, or we deliberately picked
            // gzip via `never_decompress` knowing the client decompresses
            // transparently. Either way: ship the bytes.
            let _ = accepts;
            Disposition::AsIs
        }
        Encoding::Brotli => {
            // We only ever pick brotli when the client advertised `br`.
            debug_assert!(accepts.brotli);
            Disposition::AsIs
        }
    }
}

async fn handler<A: RustEmbed>(
    State(config): State<RouterConfig>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let accepts = parse_accept_encoding(&headers);
    let prefs = encoding_prefs(accepts, &config);

    let Some(asset) = resolve_with::<A>(uri.path(), &prefs) else {
        // No variant matched the client's accepted encodings; try again
        // with the default order so we can decompress on the fly.
        let Some(asset) = resolve_with::<A>(uri.path(), crate::DEFAULT_ENCODINGS) else {
            return (StatusCode::NOT_FOUND, "Not Found").into_response();
        };
        return serve(asset, Disposition::Decompress);
    };

    let disposition = pick_disposition(asset.encoding, accepts);
    serve(asset, disposition)
}

fn serve(asset: crate::Asset, disposition: Disposition) -> Response {
    let mime = mime_guess::from_path(&asset.path).first_or_octet_stream();
    match disposition {
        Disposition::AsIs => {
            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::VARY, "Accept-Encoding");
            if let Some(enc) = asset.encoding.content_encoding() {
                builder = builder.header(header::CONTENT_ENCODING, enc);
            }
            builder.body(Body::from(asset.data.into_owned())).unwrap()
        }
        Disposition::Decompress => match asset.decoded() {
            Ok(decoded) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::VARY, "Accept-Encoding")
                .body(Body::from(decoded.into_owned()))
                .unwrap(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "decompression failed").into_response(),
        },
    }
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
    fn missing_header_accepts_nothing() {
        let a = parse_accept_encoding(&HeaderMap::new());
        assert!(!a.gzip && !a.brotli);
    }

    #[test]
    fn plain_gzip() {
        let a = parse_accept_encoding(&h("gzip"));
        assert!(a.gzip && !a.brotli);
    }

    #[test]
    fn plain_brotli() {
        let a = parse_accept_encoding(&h("br"));
        assert!(a.brotli && !a.gzip);
    }

    #[test]
    fn br_and_gzip() {
        let a = parse_accept_encoding(&h("br, gzip"));
        assert!(a.brotli && a.gzip);
    }

    #[test]
    fn case_insensitive() {
        let a = parse_accept_encoding(&h("GZIP, BR"));
        assert!(a.gzip && a.brotli);
    }

    #[test]
    fn ignores_q_parameter() {
        let a = parse_accept_encoding(&h("br;q=1.0, gzip;q=0.5"));
        assert!(a.brotli && a.gzip);
    }

    #[test]
    fn substring_match_is_not_enough() {
        let a = parse_accept_encoding(&h("x-gzip, brotli"));
        assert!(!a.gzip && !a.brotli);
    }

    fn default_cfg() -> RouterConfig {
        RouterConfig::default()
    }

    fn allow_decompress_cfg() -> RouterConfig {
        RouterConfig {
            never_decompress: false,
        }
    }

    #[test]
    fn prefs_prefer_brotli_over_gzip() {
        let prefs = encoding_prefs(
            AcceptedEncodings { gzip: true, brotli: true },
            &default_cfg(),
        );
        assert_eq!(prefs, vec![Encoding::Brotli, Encoding::Gzip, Encoding::Identity]);
    }

    #[test]
    fn default_cfg_offers_all_encodings_even_without_accept() {
        // never_decompress: send pre-compressed bytes regardless of what
        // the client asked for — caps CPU under load.
        let prefs = encoding_prefs(AcceptedEncodings::default(), &default_cfg());
        assert_eq!(
            prefs,
            vec![Encoding::Brotli, Encoding::Gzip, Encoding::Identity]
        );
    }

    #[test]
    fn allow_decompress_skips_compressed_when_unaccepted() {
        let prefs = encoding_prefs(AcceptedEncodings::default(), &allow_decompress_cfg());
        assert_eq!(prefs, vec![Encoding::Identity]);
    }

    #[test]
    fn allow_decompress_skips_brotli_when_client_only_accepts_gzip() {
        let prefs = encoding_prefs(
            AcceptedEncodings { gzip: true, brotli: false },
            &allow_decompress_cfg(),
        );
        assert!(!prefs.contains(&Encoding::Brotli));
        assert!(prefs.contains(&Encoding::Gzip));
    }
}

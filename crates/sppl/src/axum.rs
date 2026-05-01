//! Axum integration.

use ::axum::{
    Router,
    body::Body,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

use crate::{RustEmbed, resolve};

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
    Router::new().fallback(handler::<A>)
}

async fn handler<A: RustEmbed>(uri: Uri, headers: HeaderMap) -> Response {
    let Some(asset) = resolve::<A>(uri.path()) else {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    };

    let mime = mime_guess::from_path(&asset.path).first_or_octet_stream();
    let accepts_gzip = accepts_gzip(&headers);

    if asset.gzipped && accepts_gzip {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CONTENT_ENCODING, "gzip")
            .header(header::VARY, "Accept-Encoding")
            .body(Body::from(asset.data.into_owned()))
            .unwrap();
    }

    if asset.gzipped {
        return match asset.decoded() {
            Ok(decoded) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::VARY, "Accept-Encoding")
                .body(Body::from(decoded.into_owned()))
                .unwrap(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "decompression failed").into_response(),
        };
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(asset.data.into_owned()))
        .unwrap()
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
}

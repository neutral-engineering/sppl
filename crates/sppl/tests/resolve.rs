//! Integration tests for `sppl::resolve`. Exercises the four candidate paths
//! (exact, `.html`, trailing-slash `index.html`, SPA fallback) and the `.gz`
//! preference, against a small fixture tree under `tests/fixtures/static/`.

#[derive(sppl::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/tests/fixtures/static"]
#[crate_path = "sppl::rust_embed"]
struct Fixture;

fn body(asset: sppl::Asset) -> String {
    let bytes = asset.decoded().expect("decode").into_owned();
    String::from_utf8(bytes).expect("utf8")
}

#[test]
fn root_serves_index_html_via_fallback() {
    let asset = sppl::resolve::<Fixture>("/").expect("root resolves");
    assert_eq!(asset.path, "index.html");
    assert!(
        asset.encoding == sppl::Encoding::Gzip,
        "index.html.gz fixture exists, should be preferred"
    );
    assert_eq!(body(asset).trim(), "INDEX");
}

#[test]
fn empty_path_serves_index_html() {
    let asset = sppl::resolve::<Fixture>("").expect("empty resolves");
    assert_eq!(asset.path, "index.html");
}

#[test]
fn exact_path_match_wins() {
    let asset = sppl::resolve::<Fixture>("/assets/main.css").expect("css");
    assert_eq!(asset.path, "assets/main.css");
    assert!(
        asset.encoding == sppl::Encoding::Gzip,
        "main.css.gz fixture exists, should be preferred"
    );
    assert_eq!(body(asset).trim(), "MAIN_CSS");
}

#[test]
fn html_extension_is_tried_for_prerendered_routes() {
    let asset = sppl::resolve::<Fixture>("/about").expect("about");
    assert_eq!(asset.path, "about.html");
    assert_eq!(asset.encoding, sppl::Encoding::Identity, "no about.html.gz fixture");
    assert_eq!(body(asset).trim(), "ABOUT");
}

#[test]
fn trailing_slash_resolves_to_index_html() {
    let asset = sppl::resolve::<Fixture>("/app/").expect("app");
    assert_eq!(asset.path, "app/index.html");
    assert_eq!(body(asset).trim(), "APP_INDEX");
}

#[test]
fn directory_without_trailing_slash_also_resolves() {
    // `<path>/index.html` is one of the candidates regardless of trailing slash.
    let asset = sppl::resolve::<Fixture>("/app").expect("app");
    assert_eq!(asset.path, "app/index.html");
}

#[test]
fn unknown_path_falls_back_to_index_html() {
    let asset = sppl::resolve::<Fixture>("/no/such/route").expect("spa fallback");
    assert_eq!(asset.path, "index.html");
}

#[test]
fn leading_slash_is_normalized() {
    let with_slash = sppl::resolve::<Fixture>("/about").expect("with slash");
    let without_slash = sppl::resolve::<Fixture>("about").expect("without slash");
    assert_eq!(with_slash.path, without_slash.path);
}

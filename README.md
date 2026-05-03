# sppl

**sppl** _(supple)_ — embed static Svelte apps into your Rust binary.

A Svelte/SvelteKit app, built with `adapter-static`, is just a tree of
HTML/CSS/JS files. `sppl` bakes that tree into your Rust binary at compile
time and hands it to your web framework as a single `Router` (or a generic
asset lookup) that already knows how to:

- serve every file with the correct `Content-Type`,
- resolve SvelteKit `adapter-static` `<route>.html` files for prerendered
  routes,
- fall back to `index.html` for client-side SPA routes,
- store **one gzipped copy** of each compressible asset and serve it as-is
  with `Content-Encoding: gzip`, regardless of `Accept-Encoding` (modern
  clients all decompress transparently); flip
  [`RouterConfig::never_decompress`](crates/sppl/src/axum.rs) to `false`
  to opt back into on-the-fly decompression for legacy clients,
- ship as a single self-contained binary — no extra files to deploy.

## Compression

Run [`sppl::build::gzip_assets`](crates/sppl/src/build.rs) from your
`build.rs` once, after your Svelte build, and `sppl` takes care of the rest
at request time. Because only the gzipped bytes live in the binary, the
default request path is zero-CPU: every response is the stored gzipped
bytes, sent with `Content-Encoding: gzip`. Modern clients (browsers,
`curl --compressed`, every common HTTP library) decompress transparently;
the rare client that genuinely can't accept gzip can be served via
`router_with(RouterConfig { never_decompress: false })`, which restores
on-the-fly decompression with [`flate2`].

## Layout

```
crates/sppl/         # the library
examples/app/        # SvelteKit + adapter-static demo (built with deno)
examples/server/     # axum server that embeds the demo
```

## Usage

```toml
# Cargo.toml
[dependencies]
sppl  = "0.0.1"
axum  = "0.7"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use axum::{routing::get, Router};

#[derive(sppl::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../app/build"]
#[crate_path = "sppl::rust_embed"]
struct App;

#[tokio::main]
async fn main() {
    let api = Router::new()
        .route("/api/hello", get(|| async { "hello from rust" }));

    // Serve the embedded Svelte app on every other path.
    let app = api.fallback_service(sppl::axum::router::<App>());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

If you'd rather wire things up yourself, `sppl::resolve::<App>(path)` returns
the matching `(path, EmbeddedFile)` using the same lookup rules and is
framework-agnostic.

## Running the example

```bash
# from the repo root — the server's build.rs runs `deno task build` for you:
cargo run -p sppl-example-server

# …or build the svelte app yourself:
deno task --cwd=examples/app build
SPPL_SKIP_SVELTE_BUILD=1 cargo run -p sppl-example-server
```

Set `SPPL_SKIP_SVELTE_BUILD=1` to skip the build-script step (useful in CI
when the build is produced upstream).

Then open <http://127.0.0.1:3000>.

## Testing

```bash
cargo test -p sppl
```

Covers `accepts_gzip` header parsing and the `resolve` lookup rules
(exact path, `.html` extension, trailing-slash `index.html`, SPA fallback,
and `.gz` preference). Fixture files live under
`crates/sppl/tests/fixtures/static/`.

## Requirements

- Rust (1.75+ recommended) — `cargo`
- [Deno](https://deno.com) 2.x — drives the SvelteKit build via `deno task`

## License

MIT

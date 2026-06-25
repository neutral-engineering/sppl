# sppl

[![CI](https://github.com/neutral-engineering/sppl/actions/workflows/ci.yml/badge.svg)](https://github.com/neutral-engineering/sppl/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/sppl.svg)](https://crates.io/crates/sppl)
[![docs.rs](https://img.shields.io/docsrs/sppl)](https://docs.rs/sppl)

**sppl** _(supple)_ — embed static Svelte apps into your Rust binary.

A Svelte/SvelteKit app, built with `adapter-static`, is just a tree of
HTML/CSS/JS files. `sppl` bakes that tree into your Rust binary at compile
time and hands it to your web framework as a single `Router` (or a generic
asset lookup) that already knows how to:

- serve every file with the correct `Content-Type`,
- resolve SvelteKit `adapter-static` `<route>.html` files for prerendered
  routes,
- fall back to `index.html` for client-side SPA routes,
- store **pre-compressed copies** of each compressible asset (brotli and/or
  gzip) and pick the best variant per request based on `Accept-Encoding`;
  flip [`RouterConfig::never_decompress`](crates/sppl/src/axum.rs) to `false`
  to opt back into on-the-fly decompression for legacy clients,
- ship as a single self-contained binary — no extra files to deploy.

https://github.com/user-attachments/assets/010351d4-e685-4aa2-9c9e-3d1294adb904


## Compression

Run [`sppl::build::compress_assets`](crates/sppl/src/build.rs) from your
`build.rs` once, after your Svelte build, picking the algorithms you want:

```rust
// build.rs
use sppl::build::Algorithm;
sppl::build::compress_assets("../app/build", &[Algorithm::Brotli, Algorithm::Gzip]).unwrap();
```

For each compressible file, a `.br` and/or `.gz` sibling is written next to
the original. At request time `sppl` picks the best variant the client
accepts: **brotli** (typically 15–25% smaller than gzip) when the client
advertises `br`, gzip otherwise. By default, even clients that don't
advertise `gzip` get the gzipped bytes — every modern HTTP library
decompresses gzip transparently and this keeps per-request CPU at zero.
Flip [`RouterConfig::never_decompress`](crates/sppl/src/axum.rs) to `false`
to restore on-the-fly decompression for clients that truly can't accept
gzip.

`sppl::build::gzip_assets` is still available for backwards compatibility
and produces only `.gz` (and removes the original).

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
sppl  = "0.0.3"
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

## Hot-reload during frontend development

Embedding into the binary is for *release*. While iterating on the Svelte
side, run Vite's dev server directly so you get hot module reload without a
Rust rebuild:

```bash
# Frontend with HMR (separate terminal):
deno task --cwd=examples/app dev

# Rust backend (separate terminal), proxying API to whatever port Vite picks:
cargo run -p sppl-example-server
```

Then point your browser at the Vite URL (typically <http://localhost:5173>)
and have Vite proxy `/api/*` to your Rust server via its
[`server.proxy`](https://vitejs.dev/config/server-options.html#server-proxy)
config. Run `sppl::build::compress_assets` (or `gzip_assets`) only when
producing the final embedded binary.

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

use axum::{routing::get, Router};

#[derive(sppl::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../app/build"]
#[crate_path = "sppl::rust_embed"]
struct App;

#[tokio::main]
async fn main() {
    let api = Router::new().route("/api/hello", get(|| async { "hello from rust" }));

    let app = api.fallback_service(sppl::axum::router::<App>());

    // Override with `SPPL_ADDR=<ip>:<port>` (e.g. `0.0.0.0:3000`,
    // `192.168.1.42:8080`, `[::]:3000`). Defaults to localhost only.
    let addr = std::env::var("SPPL_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

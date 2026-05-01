use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::{
    Router,
    extract::Request,
    http::header,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
#[derive(sppl::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../app/build"]
#[crate_path = "sppl::rust_embed"]
struct App;

static STARTED: OnceLock<Instant> = OnceLock::new();
static REQUESTS: AtomicU64 = AtomicU64::new(0);
static ASSET_INVENTORY: OnceLock<String> = OnceLock::new();

// 30 one-second buckets, indexed by `epoch_sec % 30`. Each cell packs
// `(tag << 32) | count`, where `tag = epoch_sec & 0xFFFFFFFF` identifies
// which absolute second the bucket holds. A reader checks the tag to know
// whether the bucket is fresh or stale (i.e., from 30s ago).
static BUCKETS: [AtomicU64; 30] = [const { AtomicU64::new(0) }; 30];

fn epoch_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn record_in_bucket() {
    let now = epoch_sec();
    let idx = (now % 30) as usize;
    let bucket = &BUCKETS[idx];
    let tag = now << 32;
    loop {
        let cur = bucket.load(Ordering::Relaxed);
        let new = if cur >> 32 == now { cur + 1 } else { tag | 1 };
        if bucket
            .compare_exchange_weak(cur, new, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return;
        }
    }
}

// Counts for the last 30 whole seconds, oldest first.
fn read_buckets() -> [u32; 30] {
    let now = epoch_sec();
    let mut out = [0u32; 30];
    for offset in 0..30u64 {
        let target = now.saturating_sub(29 - offset);
        let idx = (target % 30) as usize;
        let v = BUCKETS[idx].load(Ordering::Relaxed);
        if v >> 32 == target {
            out[offset as usize] = v as u32;
        }
    }
    out
}

async fn count_requests(req: Request, next: Next) -> Response {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    record_in_bucket();
    next.run(req).await
}

async fn status() -> Response {
    let pid = std::process::id();
    let uptime_secs = STARTED.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let requests = REQUESTS.load(Ordering::Relaxed);
    let buckets = read_buckets();
    let buckets_json = buckets
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let body = format!(
        r#"{{"pid":{pid},"uptime_secs":{uptime_secs},"epoch_secs":{epoch_secs},"requests":{requests},"buckets":[{buckets_json}]}}"#
    );

    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

async fn assets() -> Response {
    let body = ASSET_INVENTORY
        .get()
        .cloned()
        .unwrap_or_else(|| r#"{"count":0,"bytes_in_binary":0,"uncompressed_bytes":0}"#.to_string());
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

// One-shot inventory of the embedded bundle. For `.gz` files we read the
// gzip trailer's ISIZE (last 4 bytes, little-endian) — exact for any payload
// under 4 GiB, which covers any plausible web bundle.
fn compute_inventory() -> String {
    let mut count: u64 = 0;
    let mut bytes_in_binary: u64 = 0;
    let mut uncompressed_bytes: u64 = 0;
    for name in App::iter() {
        let Some(file) = App::get(&name) else {
            continue;
        };
        count += 1;
        bytes_in_binary += file.data.len() as u64;
        uncompressed_bytes += if name.ends_with(".gz") && file.data.len() >= 4 {
            let n = file.data.len();
            let isize_bytes: [u8; 4] = file.data[n - 4..n].try_into().unwrap();
            u32::from_le_bytes(isize_bytes) as u64
        } else {
            file.data.len() as u64
        };
    }
    format!(
        r#"{{"count":{count},"bytes_in_binary":{bytes_in_binary},"uncompressed_bytes":{uncompressed_bytes}}}"#
    )
}

#[tokio::main]
async fn main() {
    STARTED.set(Instant::now()).ok();
    ASSET_INVENTORY.set(compute_inventory()).ok();

    let api = Router::new()
        .route("/api/hello", get(|| async { "hello from rust" }))
        .route("/api/status", get(status))
        .route("/api/assets", get(assets));

    let app = api
        .fallback_service(sppl::axum::router::<App>())
        .layer(middleware::from_fn(count_requests));

    // Override with `SPPL_ADDR=<ip>:<port>` (e.g. `0.0.0.0:3000`,
    // `192.168.1.42:8080`, `[::]:3000`). Defaults to localhost only.
    let addr = std::env::var("SPPL_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

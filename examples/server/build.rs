//! Build script: compile the Svelte app via `deno task build` so that the
//! `rust-embed` derive in `main.rs` has a populated `../app/build` directory
//! to read from, then pre-gzip the compressible files in that directory so
//! the binary stores one compressed copy of each.
//!
//! Set `SPPL_SKIP_SVELTE_BUILD=1` to skip the deno build step (e.g. on CI
//! where the build is already produced upstream). Gzip still runs.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let app_dir = manifest.join("..").join("app");
    let build_dir = app_dir.join("build");

    println!("cargo:rerun-if-env-changed=SPPL_SKIP_SVELTE_BUILD");
    println!("cargo:rerun-if-changed={}", app_dir.join("src").display());
    println!(
        "cargo:rerun-if-changed={}",
        app_dir.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        app_dir.join("svelte.config.js").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        app_dir.join("vite.config.ts").display()
    );

    if std::env::var_os("SPPL_SKIP_SVELTE_BUILD").is_none() {
        run_deno_build(&app_dir);
    }

    if !build_dir.exists() {
        panic!(
            "expected svelte build output at {} but it does not exist",
            build_dir.display()
        );
    }

    // Pre-gzip compressible assets so the binary stores one compressed copy.
    // `sppl::resolve` will serve those bytes as-is to clients that accept
    // gzip and decompress them on the fly otherwise.
    sppl::build::gzip_assets(&build_dir).expect("gzip_assets failed");
}

fn run_deno_build(app_dir: &std::path::Path) {
    let deno = which("deno").unwrap_or_else(|| {
        panic!("`deno` not found on PATH; install Deno or set SPPL_SKIP_SVELTE_BUILD=1")
    });

    let status = Command::new(&deno)
        .args(["task", "build"])
        .current_dir(app_dir)
        .status()
        .expect("failed to spawn `deno task build`");

    if !status.success() {
        panic!("`deno task build` failed with status {status}");
    }
}

fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

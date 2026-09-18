//! Build script: copy the source `config.toml` next to the compiled binary so
//! the CLI always finds its config beside itself.
//!
//! The source file (`CARGO_MANIFEST_DIR/config.toml`) is copied to
//! `target/<profile>/config.toml` on every build. If the source config does
//! not exist (e.g. a fresh clone), the build proceeds without copying.

use std::path::PathBuf;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set by cargo");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());

    let src = PathBuf::from(&manifest).join("config.toml");
    if !src.exists() {
        // No source config; the binary will use env vars or error at runtime.
        return;
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(&manifest).join("target"));
    let dst = target_dir.join(&profile).join("config.toml");

    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::copy(&src, &dst).ok();

    println!("cargo:rerun-if-changed=config.toml");
}

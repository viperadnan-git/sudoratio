//! Production-only frontend build.
//!
//! Runs `bun install` + `bun run build` against the `web/` package whenever its sources change,
//! producing `web/dist/client/` which `rust-embed` then bundles into the server binary at compile
//! time. Skipped in dev (`cargo check`, `cargo clippy`, debug builds) so day-to-day Rust work
//! doesn't pay the bun build cost — set `SUDORATIO_BUILD_WEB=1` to force it during a debug build.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let web_dir = manifest_dir.parent().expect("crate has parent").join("web");

    // Re-run only when web sources change. Listing specific roots avoids invalidating on noise
    // like editor swap files in node_modules. `cargo:rerun-if-env-changed` triggers a rebuild
    // when the toggle env var flips.
    println!("cargo:rerun-if-env-changed=SUDORATIO_BUILD_WEB");
    for sub in [
        "src",
        "public",
        "index.html",
        "package.json",
        "vite.config.ts",
        "tsconfig.json",
    ] {
        let p = web_dir.join(sub);
        if p.exists() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let force = std::env::var("SUDORATIO_BUILD_WEB").is_ok_and(|v| v != "0" && !v.is_empty());
    let dist_index = web_dir.join("dist/index.html");
    if profile != "release" && !force {
        if !dist_index.exists() {
            // No built assets and not in release mode: emit a placeholder so rust-embed has a
            // directory to read. The runtime will return 404 for every static route — fine for
            // backend-only dev workflows.
            std::fs::create_dir_all(web_dir.join("dist")).ok();
            std::fs::write(
                &dist_index,
                "<!doctype html><meta charset=utf-8><title>sudoratio</title>\
                 <p>UI not built. Run <code>cargo build --release</code> or set \
                 <code>SUDORATIO_BUILD_WEB=1</code>.</p>",
            )
            .ok();
        }
        return;
    }

    let bun = which("bun").unwrap_or_else(|| {
        panic!(
            "`bun` is required to build the web UI. Install bun (https://bun.sh) or run a debug \
             build (`cargo build`) to skip this step."
        )
    });

    run(&bun, &["install", "--frozen-lockfile"], &web_dir);
    run(&bun, &["run", "build"], &web_dir);

    if !dist_index.exists() {
        panic!(
            "web build did not produce {}; check the bun build output above",
            dist_index.display()
        );
    }
}

fn run(program: &Path, args: &[&str], cwd: &Path) {
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {} {:?}: {e}", program.display(), args));
    if !status.success() {
        panic!("{} {:?} exited with {status}", program.display(), args);
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

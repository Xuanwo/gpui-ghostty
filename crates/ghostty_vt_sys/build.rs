use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("ghostty_vt_sys must live under crates/*");

    let ghostty_dir = workspace_root.join("vendor/ghostty");
    println!("cargo:rerun-if-changed={}", ghostty_dir.join("build.zig.zon").display());

    let zig_build_enabled = std::env::var_os("CARGO_FEATURE_ZIG_BUILD").is_some();
    if !zig_build_enabled {
        return;
    }

    if !ghostty_dir.exists() {
        panic!(
            "vendor/ghostty is missing; run `git submodule update --init --recursive` and retry"
        );
    }

    let zig_version = Command::new("zig").arg("version").output().ok();
    if zig_version.is_none() {
        panic!("`zig` is required for --features zig-build, but it was not found in PATH");
    }

    // The pinned Ghostty tag (v1.2.3) does not ship a standalone `libghostty-vt` build target.
    // Keep this as a hard error so the failure mode is obvious and actionable.
    panic!(
        "Ghostty v1.2.3 does not provide a `libghostty-vt` build target yet; \
update the Ghostty submodule to a revision that exports `zig build lib-vt`, \
then implement link/bindings in crates/ghostty_vt_sys"
    );
}


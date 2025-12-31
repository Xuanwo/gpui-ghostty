# GPUI + Ghostty (VT) Terminal Control

This repository bootstraps an embedded terminal control stack:

- VT parsing/state: Ghostty (planned: `libghostty-vt`)
- Rendering/UI: GPUI (from Zed)

## Version Pinning

- Ghostty is vendored as a git submodule at `vendor/ghostty`, pinned to tag `v1.2.3`.
- GPUI is consumed from Zed using a git dependency pinned to commit `6016d0b8c6a22e586158d3b6f810b3cebb136118`.

## Development

- Initialize submodules: `git submodule update --init --recursive`
- Default build checks: `cargo check`

### Optional: Zig toolchain (required for VT core)

- Bootstrap Zig into `.context/zig`: `./scripts/bootstrap-zig.sh`
- Build VT core: `cargo check -p ghostty_vt_sys --features zig-build`
- Run VT core tests: `cargo test -p ghostty_vt --features zig-build`

### Optional: GPUI crates

GPUI-related crates are not part of the workspace default members.

- Build GPUI crate: `cargo check -p gpui_ghostty_terminal --features gpui`
- Build demo: `cargo check -p basic_terminal --features gpui`

### Optional: VT dump example

Pipe any byte stream into the VT core and print the rendered viewport text:

`printf '\033[31mred\033[0m\n' | cargo run -p vt_dump --features zig-build`

### Optional: Zig build integration (future)

`crates/ghostty_vt_sys` contains a `zig-build` feature intended to build a future `libghostty-vt`
artifact from the vendored Ghostty source using Zig.

At the pinned Ghostty version (`v1.2.3`), the `libghostty-vt` build target is not yet available,
so `--features zig-build` is expected to fail with a clear message.

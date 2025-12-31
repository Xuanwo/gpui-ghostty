# Roadmap: GPUI Embedded Terminal (libghostty-vt)

## Goal

Deliver a maintainable Rust workspace that bootstraps an embedded terminal control stack using:

- `libghostty-vt` for VT parsing/state
- GPUI as the only rendering stack (no Ghostty renderer reuse)
- Pinned upstream revisions to reduce churn

## Hard Constraints

- Ghostty is vendored via git submodule and pinned to tag `v1.2.3`.
- GPUI is consumed from Zed via git dependency pinned to commit `6016d0b8c6a22e586158d3b6f810b3cebb136118`.
- Public API surface stays minimal to tolerate upstream API churn.

## Scope (This Task)

### M0: Workspace Bootstrap (Must Finish)

- [x] Add Ghostty submodule at `vendor/ghostty` pinned to `v1.2.3`.
- [x] Create Rust workspace layout:
  - `crates/ghostty_vt_sys` (raw FFI + Zig build integration hooks)
  - `crates/ghostty_vt` (safe wrapper stubs)
  - `crates/gpui_ghostty_terminal` (GPUI control scaffolding; can be feature-gated)
  - `examples/basic_terminal` (minimal demo skeleton; can be feature-gated)
- [x] Add initial build plumbing:
  - `ghostty_vt_sys` builds without requiring Zig at compile-time by default
  - Optional `bindgen`/Zig integration path documented for developers
- [x] Provide `cargo` entrypoints:
  - `cargo check` works for the default workspace set
  - Optional `--features gpui` build path for GPUI crates
- [x] Add minimal documentation for local development and version pinning.

### M0.1: VT Core (Must Finish)

- [x] Provide a pinned Zig toolchain bootstrap script (Zig `0.14.1`) that installs into `.context/zig` (gitignored).
- [x] Implement a minimal Zig-based VT core library (built from vendored Ghostty sources) that supports:
  - terminal create/free
  - feed bytes
  - viewport dump as UTF-8
- [x] Expose the VT core library via a Rust `sys` crate API (no bindgen required).
- [x] Provide a safe Rust wrapper that can:
  - create a terminal with a given size
  - feed bytes
  - dump the viewport as `String`
- [x] Add a validation path:
  - `cargo check -p ghostty_vt_sys`
  - `cargo test -p ghostty_vt`
  
## User Work

- [x] Cleanup features: make gpui and Zig build required.

## Future Work

- M1: Incremental damage updates, selection/copy, scrollback, bracketed paste, basic mouse.
- M2: OSC title/link/clipboard, fuller keyboard encoding via Ghostty key encoder, mouse modes.
- M3: Unicode/fallback font strategy, high-throughput batching/backpressure, deep behavior regressions.

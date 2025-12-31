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

## Agent Work

### M0: Workspace Bootstrap (Must Finish)

- [x] Add Ghostty submodule at `vendor/ghostty` pinned to `v1.2.3`.
- [x] Create Rust workspace layout:
  - `crates/ghostty_vt_sys` (raw FFI + Zig build integration hooks)
  - `crates/ghostty_vt` (safe wrapper stubs)
  - `crates/gpui_ghostty_terminal` (GPUI control scaffolding)
  - `examples/basic_terminal` (minimal demo)
- [x] Add initial build plumbing:
  - `ghostty_vt_sys` builds via Zig by default (requires Zig `0.14.1`)
- [x] Provide `cargo` entrypoints:
  - `cargo check`
  - `cargo test -p ghostty_vt`
  - `cargo run -p vt_dump`
  - `cargo run -p basic_terminal`
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

### M1.1: Viewport Scrolling (Minimal)

- [x] Add a VT API to scroll the viewport by line delta.
- [x] Wire mouse-wheel scrolling in `basic_terminal`.

### M1.2: Basic Paste and Scroll Keys (Minimal)

- [x] Handle paste via clipboard read and VT feed.
- [x] Bind `cmd-v` in `basic_terminal` to trigger paste.
- [x] Support `PageUp`/`PageDown` viewport scrolling.
- [x] Avoid unconditional viewport dumps during render.

### M1.3: Basic Copy (Minimal)

- [x] Add a `Copy` action that writes the current viewport to clipboard.
- [x] Bind `cmd-c` in `basic_terminal` to trigger copy.

### M1.4: Bracketed Paste + Click-to-Focus (Minimal)

- [x] Track bracketed paste mode (`DECSET 2004`) from output bytes.
- [x] When bracketed paste mode is enabled, wrap pasted content with `ESC[200~` and `ESC[201~`.
- [x] Focus the view on mouse click.

### M1.5: Scrollback Jump (Minimal)

- [x] Add VT APIs to jump viewport to scrollback top/bottom.
- [x] Bind `Home`/`End` keys to jump viewport top/bottom.

### M2.1: OSC Title (Minimal)

- [x] Track OSC window title updates (`OSC 0` / `OSC 2`) from output bytes.
- [x] Apply the tracked title to the GPUI window title.

### M2.2: OSC Clipboard (Minimal)

- [x] Track `OSC 52` clipboard updates from output bytes.
- [x] Decode Base64 payload and write to the system clipboard.

### M2.3: Basic Key Sequences for PTY (Minimal)

- [x] Send arrow keys (`Up/Down/Left/Right`) to PTY as ANSI sequences.
- [x] Send `Escape`, `Delete`, and `Backspace` to PTY with common sequences.

### M2.4: Modifier-Aware PTY Keys (Minimal)

- [x] Send `Home`/`End`/`PageUp`/`PageDown` to PTY without modifiers; use `Shift+...` for scrollback controls.
- [x] Send function keys (`F1`..`F12`) to PTY with common xterm sequences.
- [x] Support `Alt+<char>` by prefixing with `ESC` in PTY mode.

### M4.1: PTY Login Shell Example (Minimal)

- [x] Add an example that spawns the user's login shell in a PTY.
- [x] Wire terminal input (typing + paste) to PTY stdin.
- [x] Stream PTY stdout to the VT core and render in GPUI.

### M3.1: Output Coalescing (Minimal)

- [x] Coalesce PTY output updates to reduce viewport dumps under high throughput.

## User Work

- [x] Cleanup features: make gpui and Zig build required.
- [x] Auto push to remote after commit (documented in `AGENTS.md`).
- [x] Add basic keyboard input to `basic_terminal` (type-to-echo).
- [x] Do not proactively add new `User Work` items (documented in `AGENTS.md`).
- [x] Always load and follow the latest user instructions (documented in `AGENTS.md`).
- [x] Fix `basic_terminal` rendering when text is not visible (avoid black-on-black).
- [x] Ask agents to do refactors while needed to make sure this projects can be well-maintained (documented in `AGENTS.md`).
- [x] Use Rust 2024 edition across the workspace.
- [x] Avoid over-splitting work (documented in `AGENTS.md`).

## Future Work

- M1: Incremental damage updates, selection/copy, scrollback, bracketed paste, basic mouse.
- M2: OSC title/link/clipboard, fuller keyboard encoding via Ghostty key encoder, mouse modes.
- M3: Unicode/fallback font strategy, high-throughput batching/backpressure, deep behavior regressions.
- M4: End to end examples that starts user login shell inside terminal.

# GPUI + Ghostty (VT) Embedded Terminal

This repository is a minimal, pinned, and testable embedded terminal stack:

- VT parsing/state: Ghostty's terminal core (vendored as a submodule)
- Rendering/UI: GPUI (from Zed), with a custom renderer (no Ghostty renderer reuse)

## Workspace Layout

- `crates/ghostty_vt_sys`: Zig build + C ABI for the Ghostty VT core
- `crates/ghostty_vt`: safe Rust wrapper over the C ABI
- `crates/gpui_ghostty_terminal`: GPUI `TerminalView` + input/selection/rendering glue
- `examples/vt_dump`: feed bytes into VT and print the viewport
- `examples/basic_terminal`: minimal GPUI view that renders a `TerminalSession`
- `examples/pty_terminal`: login shell PTY wired to `TerminalView`
- `examples/split_pty_terminal`: two PTYs in split panes

## Version Pinning

- Ghostty is vendored at `vendor/ghostty` and pinned to tag `v1.2.3`.
- Zig is pinned to `0.14.1` (required to build the vendored Ghostty core).
- GPUI is consumed from Zed pinned to commit `6016d0b8c6a22e586158d3b6f810b3cebb136118`.

## Build Prerequisites

1. Initialize submodules:

```sh
git submodule update --init --recursive
```

2. Install Zig (pinned) into `.context/zig/zig`:

```sh
./scripts/bootstrap-zig.sh
```

3. Build and test:

```sh
cargo test
```

Notes:

- `crates/ghostty_vt_sys` requires `zig`. If `zig` is not in `PATH`, it will use `.context/zig/zig`.
- You can also set `ZIG=/path/to/zig` to override discovery.

## Running Examples

VT dump:

```sh
printf '\033[31mred\033[0m\n' | cargo run -p vt_dump
```

GPUI demos:

```sh
cargo run -p basic_terminal
cargo run -p pty_terminal
cargo run -p split_pty_terminal
```

## Public API (gpui_ghostty_terminal)

Crate root re-exports the stable entry points:

- `TerminalConfig`
- `TerminalSession`
- `default_terminal_font`, `default_terminal_font_features`
- `view::{TerminalView, TerminalInput, Copy, Paste, SelectAll}`

Embed-friendly options:

- Disable window title updates (useful when embedding into a host app that owns titles):

```rust
use gpui_ghostty_terminal::TerminalConfig;

let config = TerminalConfig {
    update_window_title: false,
    ..TerminalConfig::default()
};
```

## Compatibility Notes

This implementation includes common terminal behaviors needed by modern TUIs:

- DSR replies (`CSI 5n` / `CSI 6n`) for cursor position/status queries
- OSC title tracking (OSC 0/2), OSC 52 clipboard write
- OSC 10/11 default foreground/background queries
- SGR mouse modes + scrollback navigation bindings
- IME composition support (commit + preedit overlay)
- DEC Special Graphics (ACS line drawing) + box drawing (procedural quads)

The examples set `TERM=xterm-256color` and `COLORTERM=truecolor` to help apps enable richer output.

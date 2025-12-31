# GPUI + Ghostty (VT) Terminal Control

This repository bootstraps an embedded terminal control stack:

- VT parsing/state: Ghostty (planned: `libghostty-vt`)
- Rendering/UI: GPUI (from Zed)

## Version Pinning

- Ghostty is vendored as a git submodule at `vendor/ghostty`, pinned to tag `v1.2.3`.
- GPUI is consumed from Zed using a git dependency pinned to commit `6016d0b8c6a22e586158d3b6f810b3cebb136118`.

## Development

- Initialize submodules: `git submodule update --init --recursive`
- Install Zig into `.context/zig`: `./scripts/bootstrap-zig.sh`
- Build checks: `cargo check`

### VT dump example

Pipe any byte stream into the VT core and print the rendered viewport text:

`printf '\033[31mred\033[0m\n' | cargo run -p vt_dump`

### GPUI demo

- Run: `cargo run -p basic_terminal`

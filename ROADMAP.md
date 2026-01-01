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

## Agent Work (Compacted)

- [x] M0: Workspace Bootstrap (Ghostty submodule, workspace layout, scripts, docs)
- [x] M0.1: VT Core (Zig build + Rust sys + safe wrapper)
- [x] M1.1: Viewport Scrolling (mouse wheel)
- [x] M1.2: Paste + Scroll Keys (cmd-v, PageUp/PageDown)
- [x] M1.3: Copy (cmd-c copies viewport)
- [x] M1.4: Bracketed Paste + Click-to-Focus
- [x] M1.5: Scrollback Jump (Home/End)
- [x] M2.1: OSC Title (OSC 0/2)
- [x] M2.2: OSC Clipboard (OSC 52)
- [x] M2.3: PTY Key Sequences (arrows/esc/delete/backspace)
- [x] M2.4: Modifier-Aware PTY Keys (function keys, Alt+char, shift for scrollback)
- [x] M3.1: Output Coalescing (reduce viewport dumps)
- [x] M3.2: Output Backpressure (bound pending buffer)
- [x] M4.1: PTY Login Shell Example
- [x] M4.2: PTY Resize (bounds observer)
- [x] M4.3: Dynamic Grid Sizing (font metrics)
- [x] M1.6: Select All + Primary Selection (cmd-a)
- [x] M2.5: PTY Ctrl-Key Encoding (punctuation)

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
- [x] Compact completed roadmap items to keep `ROADMAP.md` short (documented in `AGENTS.md`).
- [x] Keep `Agent Work` and `Future Work` aligned with the implemented code (documented in `AGENTS.md`).

## Future Work

- M1 (remaining): Incremental damage updates, selection/copy, mouse modes.
- M2 (remaining): OSC links, fuller keyboard encoding via Ghostty key encoder, mouse modes.
- M3 (remaining): Unicode/fallback font strategy, deep behavior regressions.
- M4 (remaining): Better grid sizing and layout, richer end-to-end examples.
- M5: IME support (CJK input).

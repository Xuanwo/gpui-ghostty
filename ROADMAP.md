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
- [x] M2.6: SGR Mouse Modes (buttons, motion, modifiers)
- [x] M2.7: OSC8 Hyperlinks (cmd-click copies link)
- [x] M2.8: Ghostty KeyEncoder (special keys, ctrl/alt modifiers)
- [x] M3.1: Output Coalescing (reduce viewport dumps)
- [x] M3.2: Output Backpressure (bound pending buffer)
- [x] M4.1: PTY Login Shell Example
- [x] M4.2: PTY Resize (bounds observer)
- [x] M4.3: Dynamic Grid Sizing (font metrics)
- [x] M3.4: Font Fallbacks (default terminal font)
- [x] M1.6: Select All + Primary Selection (cmd-a)
- [x] M1.7: SGR Mouse Reporting (wheel + click pass-through)
- [x] M1.8: Mouse Selection + Copy Selection (Shift+drag, cmd-c)
- [x] M1.9: Deferred Viewport Refresh (coalesce scroll/key updates)
- [x] M1.10: Viewport Dirty Rows (FFI + refresh gating)
- [x] M1.11: Incremental Viewport Updates (dirty rows -> row dumps)
- [x] M2.5: PTY Ctrl-Key Encoding (punctuation)
- [x] M3.3: Regression Tests (scrollback + resize)
- [x] M1.12: Incremental Damage Updates (reduce per-frame work)
- [x] M3.5: Deep Behavior Regression Coverage
- [x] M4.4: Layout + End-to-End Examples
- [x] M5.1: IME Support (commit + preedit)
- [x] M5.2: TUI Styling (SGR fg/bg + inverse)
- [x] M5.3: Layout Clipping and Viewport Sizing
- [x] M5.4: Monospace Alignment (disable ligatures, stable glyph positions)
- [x] M5.5: Default Monospace Font Family
- [x] M5.6: Interactive TUI Polish (clear artifacts, reduce startup cost)
- [x] M5.7: IME Preedit Overlay (avoid text overlap)
- [x] M5.8: Text Attributes (bold, faint, underline)
- [x] M5.9: Line Drawing Charset (DEC Special Graphics)
- [x] M5.10: IME Enter Passthrough (do not skip Enter key)
- [x] M5.11: Unicode Box Drawing (procedural glyph overlay)
- [x] M5.12: OSC Default Colors (reply to OSC 10/11 queries)
- [x] M5.13: Ctrl-C Cancel (send ETX for ctrl key chords)
- [x] M5.14: Startup Perf (style-run dump, avoid per-cell styles)
- [x] M5.15: Viewport Perf (avoid full viewport rebuild per chunk)
- [x] M6.1: Default Colors (configurable fg/bg for OSC 10/11 + renderer clear)
- [x] M6.2: Background Clear (always paint base background to avoid artifacts)
- [x] M6.3: Terminal Env Defaults (TERM/COLORTERM for examples)
- [x] M7.1: Refactor (split gpui_ghostty_terminal into modules)
- [x] M7.2: API Surface (stable re-exports + minimal public types)
- [x] M7.3: Tests (move + expand unit test coverage)
- [x] M7.4: README (document current module layout + API)

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
- [x] Fix CJK output alignment
- [x] Fix duplicate character input (e.g. typing "j" shows twice)
- [x] Update README
- [x] Fix DSR cursor position report (for codex/claude: "The cursor position could not be read within a normal duration")
- [x] Fix selection tracking vs mouse movement (copy/drag mismatch)
- [x] Render the cursor
- [x] Fix IME candidate window position (CJK preedit misalignment)
- [x] Improve TUI rendering for apps like `htop`
- [x] Fix cursor drift vs rendered content (error grows with more input)
- [x] Fix mouse pointer hit-testing vs rendered content
- [x] Fix `htop` display issues (cursor/DSR queries)
- [x] Fix Cmd+A select-all behavior
- [x] Auto-detect http(s) URLs on Cmd+Click
- [x] Add an embed-friendly option to disable `Window::set_window_title` updates from `TerminalView`.
- [x] Add Apache-2.0 license (LICENSE + crate metadata + docs).
- [x] Remove obsolete `.gitkeep` file.
- [x] Initialize CI (GitHub Actions).
- [x] Fix auto URL detection for `https://...` links (cmd-click).
- [x] Support precise ANSI 16-color fg/bg control (OSC 4 / OSC 104 palette updates).
- [x] Fix main CI failures and enforce pre-commit checks (documented in `AGENTS.md`).
- [x] Verify Zed pin `80433cb239e868271457ac376673a5f75bc4adb1` builds (fails by default: gpui uses `SmallVec::from_const` but `smallvec` feature `const_new` is not enabled; adding `smallvec = { version = \"1.15\", features = [\"const_new\"] }` makes it build).
- [x] Switch gpui (Zed) to workspace dependency and pin `rev = "v0.217.3"`.

## Future Work

- None.

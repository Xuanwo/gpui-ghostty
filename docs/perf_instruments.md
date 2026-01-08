# Instruments Profiling Notes (macOS)

This document captures a reproducible workflow to profile the `pty_terminal` demo with debug symbols and summarizes one representative trace to identify bottlenecks.

## Build (optimized + debuginfo)

```sh
cargo build -p pty_terminal --release \
  --config 'profile.release.debug=true' \
  --config 'profile.release.strip="none"'
```

## Record (Time Profiler)

The demo supports an optional environment variable to auto-send a command into the PTY shortly after launch:

- `GPUI_GHOSTTY_PTY_DEMO_COMMAND`: a shell command string that will be written into the PTY (a trailing newline is added if missing).

Example recording command (sustained output for sampling):

```sh
xcrun xctrace record \
  --template 'Time Profiler' \
  --time-limit 10s \
  --no-prompt \
  --output traces/pty_terminal_timeprof_yes.trace \
  --env GPUI_GHOSTTY_PTY_DEMO_COMMAND='yes 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef' \
  --launch -- ./target/release/pty_terminal
```

To inspect interactively, open the resulting trace in Instruments:

```sh
open traces/pty_terminal_timeprof_yes.trace
```

## Findings (trace: `traces/pty_terminal_timeprof_yes.trace`)

Workload: continuous output (`yes ...`) driving VT parsing + state updates.

In the steady-state window (after ~2s), the top *leaf* (self-time proxy) samples in `pty_terminal` are dominated by VT feed and cell update paths:

- `_lib.Handler.print` (Zig terminal print/cell update hot path)
- `ghostty_vt_terminal_feed` / `ghostty_vt::Terminal::feed` (FFI + VT feed loop)
- `gpui_ghostty_terminal::session::TerminalSession::feed_with_pty_responses` (session feed integration)
- `gpui_ghostty_terminal::session::OscQueryScanState::advance` (OSC query scanning overhead)

The main implication is that, under high-throughput output, the bottleneck is currently on the *ingest/feed* path (VT parsing + terminal state mutation) rather than on rendering/shaping.

## Next Optimization Targets (ordered)

1. Reduce per-codepoint work in the Zig terminal cell update hot path (e.g. `Page.swapCells`/related routines).
2. Reduce `OscQueryScanState::advance` overhead by tightening scan ranges and avoiding repeated linear scans on large chunks.
3. Avoid feeding unlimited output on the UI thread: apply a per-frame budget and/or move feed onto a worker thread, then publish coalesced state/damage to the UI thread.


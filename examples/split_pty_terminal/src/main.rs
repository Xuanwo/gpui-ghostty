use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gpui::{
    App, Application, CursorStyle, Entity, KeyBinding, SharedString, Window, WindowOptions, div,
    prelude::*, px, rgba,
};
use gpui_ghostty_terminal::view::{Copy, Paste, SelectAll, TerminalInput, TerminalView};
use gpui_ghostty_terminal::{TerminalConfig, TerminalSession, default_terminal_font};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

struct Pane {
    view: Entity<TerminalView>,
    master: Arc<dyn portable_pty::MasterPty + Send>,
    stdout_rx: mpsc::Receiver<Vec<u8>>,
}

fn spawn_shell_pane(cx: &mut gpui::App) -> Pane {
    let config = TerminalConfig::default();

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty failed");

    let master: Arc<dyn portable_pty::MasterPty + Send> = Arc::from(pty_pair.master);

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = CommandBuilder::new(shell);
    cmd.arg("-l");
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "gpui-ghostty");

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .expect("spawn login shell failed");

    thread::spawn(move || {
        let _ = child.wait();
    });

    let mut pty_reader = master.try_clone_reader().expect("pty reader");
    let mut pty_writer = master.take_writer().expect("pty writer");

    let (stdin_tx, stdin_rx) = mpsc::channel::<Vec<u8>>();
    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>();

    thread::spawn(move || {
        while let Ok(bytes) = stdin_rx.recv() {
            if pty_writer.write_all(&bytes).is_err() {
                break;
            }
            let _ = pty_writer.flush();
        }
    });

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let n = match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let _ = stdout_tx.send(buf[..n].to_vec());
        }
    });

    let view = cx.new(|cx| {
        let focus_handle = cx.focus_handle();
        let session = TerminalSession::new(config).expect("vt init");
        let input = TerminalInput::new(move |bytes| {
            let _ = stdin_tx.send(bytes.to_vec());
        });
        TerminalView::new_with_input(session, focus_handle, input)
    });

    Pane {
        view,
        master,
        stdout_rx,
    }
}

fn compute_cell_metrics(window: &mut Window) -> Option<(f32, f32)> {
    let mut style = window.text_style();
    let font = default_terminal_font();
    style.font_family = font.family.clone();
    style.font_features = gpui_ghostty_terminal::default_terminal_font_features();
    style.font_fallbacks = font.fallbacks.clone();

    let rem_size = window.rem_size();
    let font_size = style.font_size.to_pixels(rem_size);
    let line_height = style.line_height.to_pixels(style.font_size, rem_size);

    let run = style.to_run(1);
    let lines = window
        .text_system()
        .shape_text(SharedString::from("M"), font_size, &[run], None, Some(1))
        .ok()?;
    let line = lines.first()?;

    let cell_width = f32::from(line.width()).max(1.0);
    let cell_height = f32::from(line_height).max(1.0);
    Some((cell_width, cell_height))
}

struct SplitTerminal {
    left: Entity<TerminalView>,
    right: Entity<TerminalView>,
}

impl Render for SplitTerminal {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_row()
            .cursor(CursorStyle::IBeam)
            .child(div().flex_1().h_full().child(self.left.clone()))
            .child(div().w(px(1.)).h_full().bg(rgba(0x404040ff)))
            .child(div().flex_1().h_full().child(self.right.clone()))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-v", Paste, None),
        ]);

        cx.open_window(WindowOptions::default(), |window, cx| {
            let left = spawn_shell_pane(cx);
            let right = spawn_shell_pane(cx);

            let left_view = left.view.clone();
            let right_view = right.view.clone();
            let left_view_for_resize = left_view.clone();
            let right_view_for_resize = right_view.clone();
            let left_view_for_task = left_view.clone();
            let right_view_for_task = right_view.clone();

            let left_master = left.master.clone();
            let right_master = right.master.clone();

            let left_rx = left.stdout_rx;
            let right_rx = right.stdout_rx;

            let split = cx.new(|_| SplitTerminal {
                left: left_view.clone(),
                right: right_view.clone(),
            });

            let subscription = split.update(cx, |_, cx| {
                cx.observe_window_bounds(window, move |_, window, cx| {
                    let size = window.viewport_size();
                    let width = f32::from(size.width).max(1.0);
                    let height = f32::from(size.height).max(1.0);

                    let Some((cell_width, cell_height)) = compute_cell_metrics(window) else {
                        return;
                    };

                    let divider_width = 1.0f32;
                    let pane_width = ((width - divider_width) / 2.0).max(1.0);

                    let cols = (pane_width / cell_width).floor().max(1.0) as u16;
                    let rows = (height / cell_height).floor().max(1.0) as u16;

                    let _ = left_master.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                    let _ = right_master.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });

                    left_view_for_resize.update(cx, |this, cx| this.resize_terminal(cols, rows, cx));
                    right_view_for_resize.update(cx, |this, cx| this.resize_terminal(cols, rows, cx));
                })
            });
            subscription.detach();

            window
                .spawn(cx, async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;

                        let mut left_batch = Vec::new();
                        while let Ok(chunk) = left_rx.try_recv() {
                            left_batch.extend_from_slice(&chunk);
                        }
                        if !left_batch.is_empty() {
                            cx.update(|_, cx| {
                                left_view_for_task.update(cx, |this, cx| {
                                    this.queue_output_bytes(&left_batch, cx);
                                });
                            })
                            .ok();
                        }

                        let mut right_batch = Vec::new();
                        while let Ok(chunk) = right_rx.try_recv() {
                            right_batch.extend_from_slice(&chunk);
                        }
                        if !right_batch.is_empty() {
                            cx.update(|_, cx| {
                                right_view_for_task.update(cx, |this, cx| {
                                    this.queue_output_bytes(&right_batch, cx);
                                });
                            })
                            .ok();
                        }
                    }
                })
                .detach();

            split
        })
        .unwrap();
    });
}

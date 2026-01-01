use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gpui::{App, AppContext, Application, KeyBinding, SharedString, WindowOptions};
use gpui_ghostty_terminal::view::{Copy, Paste, SelectAll, TerminalInput, TerminalView};
use gpui_ghostty_terminal::{TerminalConfig, TerminalSession};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-v", Paste, None),
        ]);

        cx.open_window(WindowOptions::default(), |window, cx| {
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
                focus_handle.focus(window);

                let session = TerminalSession::new(config).expect("vt init");
                let input = TerminalInput::new(move |bytes| {
                    let _ = stdin_tx.send(bytes.to_vec());
                });

                TerminalView::new_with_input(session, focus_handle, input)
            });

            let master_for_resize = master.clone();
            let subscription = view.update(cx, |_, cx| {
                cx.observe_window_bounds(window, move |this, window, cx| {
                    let size = window.viewport_size();
                    let width = f32::from(size.width);
                    let height = f32::from(size.height);

                    let mut style = window.text_style();
                    let font = gpui_ghostty_terminal::default_terminal_font();
                    style.font_family = font.family.clone();
                    style.font_features = gpui_ghostty_terminal::default_terminal_font_features();
                    style.font_fallbacks = font.fallbacks.clone();

                    let rem_size = window.rem_size();
                    let font_size = style.font_size.to_pixels(rem_size);
                    let line_height = style.line_height.to_pixels(style.font_size, rem_size);

                    let run = style.to_run(1);
                    let Ok(lines) = window.text_system().shape_text(
                        SharedString::from("M"),
                        font_size,
                        &[run],
                        None,
                        Some(1),
                    ) else {
                        return;
                    };
                    let Some(line) = lines.first() else {
                        return;
                    };

                    let cell_width = f32::from(line.width()).max(1.0);
                    let cell_height = f32::from(line_height).max(1.0);

                    let cols = (width / cell_width).floor().max(1.0) as u16;
                    let rows = (height / cell_height).floor().max(1.0) as u16;

                    let _ = master_for_resize.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    });

                    this.resize_terminal(cols, rows, cx);
                })
            });
            subscription.detach();

            let view_for_task = view.clone();
            window
                .spawn(cx, async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                        let mut batch = Vec::new();
                        while let Ok(chunk) = stdout_rx.try_recv() {
                            batch.extend_from_slice(&chunk);
                        }
                        if batch.is_empty() {
                            continue;
                        }

                        cx.update(|_, cx| {
                            view_for_task.update(cx, |this, cx| {
                                this.queue_output_bytes(&batch, cx);
                            });
                        })
                        .ok();
                    }
                })
                .detach();

            view
        })
        .unwrap();
    });
}

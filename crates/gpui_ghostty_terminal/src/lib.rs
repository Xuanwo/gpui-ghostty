use ghostty_vt::{Error, Terminal};

#[derive(Clone, Copy, Debug)]
pub struct TerminalConfig {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

pub struct TerminalSession {
    config: TerminalConfig,
    terminal: Terminal,
    bracketed_paste_enabled: bool,
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        Ok(Self {
            config,
            terminal: Terminal::new(config.cols, config.rows)?,
            bracketed_paste_enabled: false,
        })
    }

    pub fn cols(&self) -> u16 {
        self.config.cols
    }

    pub fn rows(&self) -> u16 {
        self.config.rows
    }

    pub fn bracketed_paste_enabled(&self) -> bool {
        self.bracketed_paste_enabled
    }

    fn update_modes_from_output(&mut self, bytes: &[u8]) {
        const ENABLE: &[u8] = b"\x1b[?2004h";
        const DISABLE: &[u8] = b"\x1b[?2004l";

        let mut i = 0usize;
        while i + 3 < bytes.len() {
            if bytes[i] == 0x1b && bytes[i + 1] == b'[' && bytes[i + 2] == b'?' {
                let tail = &bytes[i..];
                if tail.starts_with(ENABLE) {
                    self.bracketed_paste_enabled = true;
                    i += ENABLE.len();
                    continue;
                }
                if tail.starts_with(DISABLE) {
                    self.bracketed_paste_enabled = false;
                    i += DISABLE.len();
                    continue;
                }
            }
            i += 1;
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.update_modes_from_output(bytes);
        self.terminal.feed(bytes)
    }

    pub fn dump_viewport(&self) -> Result<String, Error> {
        self.terminal.dump_viewport()
    }

    pub fn scroll_viewport(&mut self, delta_lines: i32) -> Result<(), Error> {
        self.terminal.scroll_viewport(delta_lines)
    }
}

pub mod view {
    use super::TerminalSession;
    use gpui::{
        actions, div, prelude::*, ClipboardItem, Context, FocusHandle, IntoElement, KeyDownEvent,
        MouseButton, MouseDownEvent, Render, ScrollDelta, ScrollWheelEvent, Window,
    };

    actions!(terminal_view, [Copy, Paste]);

    pub struct TerminalView {
        session: TerminalSession,
        viewport: String,
        focus_handle: FocusHandle,
    }

    impl TerminalView {
        pub fn new(session: TerminalSession, focus_handle: FocusHandle) -> Self {
            Self {
                session,
                viewport: String::new(),
                focus_handle,
            }
            .with_refreshed_viewport()
        }

        fn with_refreshed_viewport(mut self) -> Self {
            self.refresh_viewport();
            self
        }

        fn refresh_viewport(&mut self) {
            self.viewport = self.session.dump_viewport().unwrap_or_default();
        }

        fn on_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
            let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                return;
            };

            if self.session.bracketed_paste_enabled() {
                let _ = self.session.feed(b"\x1b[200~");
                let _ = self.session.feed(text.as_bytes());
                let _ = self.session.feed(b"\x1b[201~");
            } else {
                let _ = self.session.feed(text.as_bytes());
            }
            self.refresh_viewport();
            cx.notify();
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            cx.write_to_clipboard(ClipboardItem::new_string(self.viewport.clone()));
        }

        fn on_mouse_down(
            &mut self,
            _: &MouseDownEvent,
            window: &mut Window,
            _cx: &mut Context<Self>,
        ) {
            self.focus_handle.focus(window);
        }

        fn on_key_down(
            &mut self,
            event: &KeyDownEvent,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            let keystroke = event.keystroke.clone().with_simulated_ime();

            if keystroke.modifiers.platform
                || keystroke.modifiers.control
                || keystroke.modifiers.alt
                || keystroke.modifiers.function
            {
                return;
            }

            let scroll_step = (self.session.rows() as i32 / 2).max(1);
            match keystroke.key.as_str() {
                "pageup" | "page_up" | "page-up" => {
                    let _ = self.session.scroll_viewport(-scroll_step);
                    self.refresh_viewport();
                    cx.notify();
                    return;
                }
                "pagedown" | "page_down" | "page-down" => {
                    let _ = self.session.scroll_viewport(scroll_step);
                    self.refresh_viewport();
                    cx.notify();
                    return;
                }
                _ => {}
            }

            if let Some(text) = keystroke.key_char.as_deref() {
                let _ = self.session.feed(text.as_bytes());
                self.refresh_viewport();
                cx.notify();
                return;
            }

            if keystroke.key == "backspace" {
                let _ = self.session.feed(&[0x08]);
                self.refresh_viewport();
                cx.notify();
            }
        }

        fn on_scroll_wheel(
            &mut self,
            event: &ScrollWheelEvent,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            let dy_lines: f32 = match event.delta {
                ScrollDelta::Lines(p) => p.y,
                ScrollDelta::Pixels(p) => f32::from(p.y) / 16.0,
            };

            let delta_lines = (-dy_lines).round() as i32;
            if delta_lines == 0 {
                return;
            }

            let _ = self.session.scroll_viewport(delta_lines);
            self.refresh_viewport();
            cx.notify();
        }
    }

    impl Render for TerminalView {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_copy))
                .on_action(cx.listener(Self::on_paste))
                .on_key_down(cx.listener(Self::on_key_down))
                .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .font_family("monospace")
                .whitespace_nowrap()
                .child(self.viewport.clone())
        }
    }
}

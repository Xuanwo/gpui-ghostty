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
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        Ok(Self {
            config,
            terminal: Terminal::new(config.cols, config.rows)?,
        })
    }

    pub fn cols(&self) -> u16 {
        self.config.cols
    }

    pub fn rows(&self) -> u16 {
        self.config.rows
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
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
        Render, ScrollDelta, ScrollWheelEvent, Window,
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

            let _ = self.session.feed(text.as_bytes());
            self.refresh_viewport();
            cx.notify();
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            cx.write_to_clipboard(ClipboardItem::new_string(self.viewport.clone()));
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
                .font_family("monospace")
                .whitespace_nowrap()
                .child(self.viewport.clone())
        }
    }
}

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
    title: Option<String>,
    clipboard_write: Option<String>,
    parse_tail: Vec<u8>,
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        Ok(Self {
            config,
            terminal: Terminal::new(config.cols, config.rows)?,
            bracketed_paste_enabled: false,
            title: None,
            clipboard_write: None,
            parse_tail: Vec::new(),
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

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn take_clipboard_write(&mut self) -> Option<String> {
        self.clipboard_write.take()
    }

    fn update_state_from_output(&mut self, bytes: &[u8]) {
        const ENABLE: &[u8] = b"\x1b[?2004h";
        const DISABLE: &[u8] = b"\x1b[?2004l";
        const TAIL_LIMIT: usize = 2048;

        self.parse_tail.extend_from_slice(bytes);
        if self.parse_tail.len() > TAIL_LIMIT {
            let drop_len = self.parse_tail.len() - TAIL_LIMIT;
            self.parse_tail.drain(0..drop_len);
        }
        let buf = self.parse_tail.as_slice();

        let mut i = 0usize;
        while i + 3 < buf.len() {
            if buf[i] == 0x1b && buf[i + 1] == b'[' && buf[i + 2] == b'?' {
                let tail = &buf[i..];
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

        let mut last_title: Option<String> = None;
        let mut last_clipboard: Option<String> = None;
        let mut j = 0usize;
        while j + 1 < buf.len() {
            if buf[j] != 0x1b || buf[j + 1] != b']' {
                j += 1;
                continue;
            }

            let mut k = j + 2;
            let mut ps: u32 = 0;
            let mut saw_digit = false;
            while k < buf.len() {
                let b = buf[k];
                if b.is_ascii_digit() {
                    saw_digit = true;
                    ps = ps.saturating_mul(10).saturating_add((b - b'0') as u32);
                    k += 1;
                    continue;
                }
                if b == b';' {
                    k += 1;
                    break;
                }
                break;
            }
            if !saw_digit || k >= buf.len() {
                j += 1;
                continue;
            }

            let title_start = k;
            while k < buf.len() {
                match buf[k] {
                    0x07 => {
                        if ps == 0 || ps == 2 {
                            last_title =
                                Some(String::from_utf8_lossy(&buf[title_start..k]).into_owned());
                        } else if ps == 52 {
                            last_clipboard = decode_osc_52(&buf[title_start..k]);
                        }
                        k += 1;
                        break;
                    }
                    0x1b if k + 1 < buf.len() && buf[k + 1] == b'\\' => {
                        if ps == 0 || ps == 2 {
                            last_title =
                                Some(String::from_utf8_lossy(&buf[title_start..k]).into_owned());
                        } else if ps == 52 {
                            last_clipboard = decode_osc_52(&buf[title_start..k]);
                        }
                        k += 2;
                        break;
                    }
                    _ => k += 1,
                }
            }

            j = k.max(j + 1);
        }

        if let Some(title) = last_title {
            self.title = Some(title);
        }
        if let Some(clipboard) = last_clipboard {
            self.clipboard_write = Some(clipboard);
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.update_state_from_output(bytes);
        self.terminal.feed(bytes)
    }

    pub fn dump_viewport(&self) -> Result<String, Error> {
        self.terminal.dump_viewport()
    }

    pub fn scroll_viewport(&mut self, delta_lines: i32) -> Result<(), Error> {
        self.terminal.scroll_viewport(delta_lines)
    }

    pub fn scroll_viewport_top(&mut self) -> Result<(), Error> {
        self.terminal.scroll_viewport_top()
    }

    pub fn scroll_viewport_bottom(&mut self) -> Result<(), Error> {
        self.terminal.scroll_viewport_bottom()
    }
}

pub mod view {
    use super::TerminalSession;
    use gpui::{
        ClipboardItem, Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton,
        MouseDownEvent, Render, ScrollDelta, ScrollWheelEvent, Window, actions, div, prelude::*,
    };

    actions!(terminal_view, [Copy, Paste]);

    pub struct TerminalInput {
        send: Box<dyn Fn(&[u8]) + Send + Sync + 'static>,
    }

    impl TerminalInput {
        pub fn new(send: impl Fn(&[u8]) + Send + Sync + 'static) -> Self {
            Self {
                send: Box::new(send),
            }
        }

        pub fn send(&self, bytes: &[u8]) {
            (self.send)(bytes);
        }
    }

    pub struct TerminalView {
        session: TerminalSession,
        viewport: String,
        focus_handle: FocusHandle,
        last_window_title: Option<String>,
        input: Option<TerminalInput>,
    }

    impl TerminalView {
        pub fn new(session: TerminalSession, focus_handle: FocusHandle) -> Self {
            Self {
                session,
                viewport: String::new(),
                focus_handle,
                last_window_title: None,
                input: None,
            }
            .with_refreshed_viewport()
        }

        pub fn new_with_input(
            session: TerminalSession,
            focus_handle: FocusHandle,
            input: TerminalInput,
        ) -> Self {
            Self {
                session,
                viewport: String::new(),
                focus_handle,
                last_window_title: None,
                input: Some(input),
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

        fn apply_side_effects(&mut self, cx: &mut Context<Self>) {
            if let Some(text) = self.session.take_clipboard_write() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
        }

        pub fn feed_output_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
            let _ = self.session.feed(bytes);
            self.refresh_viewport();
            self.apply_side_effects(cx);
            cx.notify();
        }

        fn on_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
            let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
                return;
            };

            if let Some(input) = self.input.as_ref() {
                if self.session.bracketed_paste_enabled() {
                    input.send(b"\x1b[200~");
                    input.send(text.as_bytes());
                    input.send(b"\x1b[201~");
                } else {
                    input.send(text.as_bytes());
                }
                return;
            }

            if self.session.bracketed_paste_enabled() {
                let _ = self.session.feed(b"\x1b[200~");
                let _ = self.session.feed(text.as_bytes());
                let _ = self.session.feed(b"\x1b[201~");
            } else {
                let _ = self.session.feed(text.as_bytes());
            }
            self.refresh_viewport();
            self.apply_side_effects(cx);
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
                || keystroke.modifiers.alt
                || keystroke.modifiers.function
            {
                return;
            }

            if keystroke.modifiers.control {
                if let Some(text) = keystroke.key_char.as_deref() {
                    let bytes = text.as_bytes();
                    if bytes.len() == 1 {
                        let b = bytes[0];
                        let b = match b {
                            b'a'..=b'z' => b - b'a' + 1,
                            b'A'..=b'Z' => b - b'A' + 1,
                            _ => return,
                        };

                        if let Some(input) = self.input.as_ref() {
                            input.send(&[b]);
                            return;
                        }

                        let _ = self.session.feed(&[b]);
                        self.refresh_viewport();
                        self.apply_side_effects(cx);
                        cx.notify();
                    }
                }
                return;
            }

            let scroll_step = (self.session.rows() as i32 / 2).max(1);
            match keystroke.key.as_str() {
                "home" => {
                    let _ = self.session.scroll_viewport_top();
                    self.refresh_viewport();
                    self.apply_side_effects(cx);
                    cx.notify();
                    return;
                }
                "end" => {
                    let _ = self.session.scroll_viewport_bottom();
                    self.refresh_viewport();
                    self.apply_side_effects(cx);
                    cx.notify();
                    return;
                }
                "pageup" | "page_up" | "page-up" => {
                    let _ = self.session.scroll_viewport(-scroll_step);
                    self.refresh_viewport();
                    self.apply_side_effects(cx);
                    cx.notify();
                    return;
                }
                "pagedown" | "page_down" | "page-down" => {
                    let _ = self.session.scroll_viewport(scroll_step);
                    self.refresh_viewport();
                    self.apply_side_effects(cx);
                    cx.notify();
                    return;
                }
                "enter" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\r");
                        return;
                    }
                }
                "tab" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\t");
                        return;
                    }
                }
                _ => {}
            }

            if let Some(text) = keystroke.key_char.as_deref() {
                if let Some(input) = self.input.as_ref() {
                    input.send(text.as_bytes());
                    return;
                }
                let _ = self.session.feed(text.as_bytes());
                self.refresh_viewport();
                self.apply_side_effects(cx);
                cx.notify();
                return;
            }

            if keystroke.key == "backspace" {
                if let Some(input) = self.input.as_ref() {
                    input.send(&[0x08]);
                    return;
                }
                let _ = self.session.feed(&[0x08]);
                self.refresh_viewport();
                self.apply_side_effects(cx);
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
            self.apply_side_effects(cx);
            cx.notify();
        }
    }

    impl Render for TerminalView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let title = self
                .session
                .title()
                .unwrap_or("GPUI Embedded Terminal (Ghostty VT)");

            if self.last_window_title.as_deref() != Some(title) {
                window.set_window_title(title);
                self.last_window_title = Some(title.to_string());
            }

            div()
                .size_full()
                .flex()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::on_copy))
                .on_action(cx.listener(Self::on_paste))
                .on_key_down(cx.listener(Self::on_key_down))
                .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .bg(gpui::black())
                .text_color(gpui::white())
                .font_family("monospace")
                .whitespace_nowrap()
                .child(self.viewport.clone())
        }
    }
}

fn decode_osc_52(payload: &[u8]) -> Option<String> {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;

    let mut split = payload.splitn(2, |b| *b == b';');
    let selection = split.next()?;
    let data = split.next()?;

    if !selection.iter().any(|b| *b == b'c') {
        return None;
    }
    if data.is_empty() {
        return None;
    }

    let decoded = STANDARD.decode(data).ok()?;
    Some(String::from_utf8_lossy(&decoded).into_owned())
}

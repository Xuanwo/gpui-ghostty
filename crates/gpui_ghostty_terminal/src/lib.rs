use ghostty_vt::{Error, Terminal};

fn update_viewport_string(current: &mut String, updated: String) -> bool {
    if *current == updated {
        false
    } else {
        *current = updated;
        true
    }
}

fn sgr_mouse_button_value(
    base_button: u8,
    motion: bool,
    shift: bool,
    alt: bool,
    control: bool,
) -> u8 {
    let mut value = base_button;
    if motion {
        value = value.saturating_add(32);
    }
    if shift {
        value = value.saturating_add(4);
    }
    if alt {
        value = value.saturating_add(8);
    }
    if control {
        value = value.saturating_add(16);
    }
    value
}

fn sgr_mouse_sequence(button_value: u8, col: u16, row: u16, pressed: bool) -> String {
    let suffix = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", button_value, col, row, suffix)
}

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
    mouse_x10_enabled: bool,
    mouse_button_event_enabled: bool,
    mouse_any_event_enabled: bool,
    mouse_sgr_enabled: bool,
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
            mouse_x10_enabled: false,
            mouse_button_event_enabled: false,
            mouse_any_event_enabled: false,
            mouse_sgr_enabled: false,
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

    pub fn mouse_reporting_enabled(&self) -> bool {
        self.mouse_x10_enabled || self.mouse_button_event_enabled || self.mouse_any_event_enabled
    }

    pub fn mouse_sgr_enabled(&self) -> bool {
        self.mouse_sgr_enabled
    }

    pub fn mouse_button_event_enabled(&self) -> bool {
        self.mouse_button_event_enabled
    }

    pub fn mouse_any_event_enabled(&self) -> bool {
        self.mouse_any_event_enabled
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn take_clipboard_write(&mut self) -> Option<String> {
        self.clipboard_write.take()
    }

    fn update_state_from_output(&mut self, bytes: &[u8]) {
        const TAIL_LIMIT: usize = 2048;

        self.parse_tail.extend_from_slice(bytes);
        if self.parse_tail.len() > TAIL_LIMIT {
            let drop_len = self.parse_tail.len() - TAIL_LIMIT;
            self.parse_tail.drain(0..drop_len);
        }
        let buf = self.parse_tail.as_slice();

        let mut i = 0usize;
        while i + 2 < buf.len() {
            if buf[i] != 0x1b || buf[i + 1] != b'[' || buf[i + 2] != b'?' {
                i += 1;
                continue;
            }

            let mut k = i + 3;
            let mut nums: Vec<u32> = Vec::new();
            let mut num: u32 = 0;
            let mut saw_digit = false;
            let mut consumed = false;

            while k < buf.len() {
                let b = buf[k];
                if b.is_ascii_digit() {
                    saw_digit = true;
                    num = num.saturating_mul(10).saturating_add((b - b'0') as u32);
                    k += 1;
                    continue;
                }

                if b == b';' {
                    if saw_digit {
                        nums.push(num);
                        num = 0;
                        saw_digit = false;
                    }
                    k += 1;
                    continue;
                }

                if b == b'h' || b == b'l' {
                    if saw_digit {
                        nums.push(num);
                    }

                    let enabled = b == b'h';
                    for ps in nums {
                        match ps {
                            2004 => self.bracketed_paste_enabled = enabled,
                            1000 => self.mouse_x10_enabled = enabled,
                            1002 => self.mouse_button_event_enabled = enabled,
                            1003 => self.mouse_any_event_enabled = enabled,
                            1006 => self.mouse_sgr_enabled = enabled,
                            _ => {}
                        }
                    }

                    i = k + 1;
                    consumed = true;
                    break;
                }

                i += 1;
                consumed = true;
                break;
            }

            if k >= buf.len() && !consumed {
                break;
            }

            if consumed {
                continue;
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

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        self.config = TerminalConfig { cols, rows };
        self.terminal.resize(cols, rows)
    }
}

pub mod view {
    use super::TerminalSession;
    use gpui::{
        ClipboardItem, Context, FocusHandle, HighlightStyle, IntoElement, KeyDownEvent,
        MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, ScrollDelta,
        ScrollWheelEvent, StyledText, Window, actions, div, prelude::*,
    };
    use std::ops::Range;

    actions!(terminal_view, [Copy, Paste, SelectAll]);

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
        pending_output: Vec<u8>,
        pending_refresh: bool,
        selection: Option<ByteSelection>,
    }

    #[derive(Clone, Copy, Debug)]
    struct ByteSelection {
        anchor: usize,
        active: usize,
    }

    impl ByteSelection {
        fn range(self) -> Range<usize> {
            if self.anchor <= self.active {
                self.anchor..self.active
            } else {
                self.active..self.anchor
            }
        }
    }

    impl TerminalView {
        pub fn new(session: TerminalSession, focus_handle: FocusHandle) -> Self {
            Self {
                session,
                viewport: String::new(),
                focus_handle,
                last_window_title: None,
                input: None,
                pending_output: Vec::new(),
                pending_refresh: false,
                selection: None,
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
                pending_output: Vec::new(),
                pending_refresh: false,
                selection: None,
            }
            .with_refreshed_viewport()
        }

        fn with_refreshed_viewport(mut self) -> Self {
            self.refresh_viewport();
            self
        }

        fn refresh_viewport(&mut self) {
            let viewport = self.session.dump_viewport().unwrap_or_default();
            if crate::update_viewport_string(&mut self.viewport, viewport) {
                self.selection = None;
            }
        }

        fn schedule_viewport_refresh(&mut self, cx: &mut Context<Self>) {
            self.pending_refresh = true;
            cx.notify();
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

        pub fn queue_output_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
            const MAX_PENDING_OUTPUT_BYTES: usize = 256 * 1024;

            if self.pending_output.len().saturating_add(bytes.len()) <= MAX_PENDING_OUTPUT_BYTES {
                self.pending_output.extend_from_slice(bytes);
                cx.notify();
                return;
            }

            if !self.pending_output.is_empty() {
                let pending = std::mem::take(&mut self.pending_output);
                let _ = self.session.feed(&pending);
                self.apply_side_effects(cx);
                self.pending_refresh = true;
            }

            if bytes.len() > MAX_PENDING_OUTPUT_BYTES {
                let mut offset = 0usize;
                while offset < bytes.len() {
                    let end = (offset + MAX_PENDING_OUTPUT_BYTES).min(bytes.len());
                    let _ = self.session.feed(&bytes[offset..end]);
                    offset = end;
                }
                self.apply_side_effects(cx);
                self.pending_refresh = true;
                cx.notify();
                return;
            }

            self.pending_output.extend_from_slice(bytes);
            cx.notify();
        }

        pub fn resize_terminal(&mut self, cols: u16, rows: u16, cx: &mut Context<Self>) {
            let _ = self.session.resize(cols, rows);
            self.pending_refresh = true;
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
            self.apply_side_effects(cx);
            self.schedule_viewport_refresh(cx);
        }

        fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
            let selection = self
                .selection
                .map(|s| s.range())
                .filter(|range| !range.is_empty())
                .and_then(|range| self.viewport.get(range))
                .unwrap_or(self.viewport.as_str());

            let item = ClipboardItem::new_string(selection.to_string());
            cx.write_to_clipboard(item.clone());
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            cx.write_to_primary(item);
        }

        fn on_select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
            self.selection = Some(ByteSelection {
                anchor: 0,
                active: self.viewport.len(),
            });
            self.on_copy(&Copy, window, cx);
        }

        fn on_mouse_down(
            &mut self,
            event: &MouseDownEvent,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.focus_handle.focus(window);

            if event.first_mouse {
                return;
            }

            if event.modifiers.shift
                || self.input.is_none()
                || !self.session.mouse_reporting_enabled()
                || !self.session.mouse_sgr_enabled()
            {
                if event.button == MouseButton::Left {
                    if let Some(index) =
                        self.mouse_position_to_viewport_index(event.position, window)
                    {
                        self.selection = Some(ByteSelection {
                            anchor: index,
                            active: index,
                        });
                        cx.notify();
                    }
                }
                return;
            }

            let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                return;
            };

            if let Some(input) = self.input.as_ref() {
                let base_button = match event.button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };

                let button_value = crate::sgr_mouse_button_value(
                    base_button,
                    false,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                let seq = crate::sgr_mouse_sequence(button_value, col, row, true);
                input.send(seq.as_bytes());
            }
        }

        fn on_mouse_up(
            &mut self,
            event: &MouseUpEvent,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if event.modifiers.shift
                || self.input.is_none()
                || !self.session.mouse_reporting_enabled()
                || !self.session.mouse_sgr_enabled()
            {
                if let Some(selection) = self.selection {
                    if selection.range().is_empty() {
                        self.selection = None;
                    }
                    cx.notify();
                }
                return;
            }

            let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                return;
            };

            if let Some(input) = self.input.as_ref() {
                let base_button = match event.button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };

                let button_value = crate::sgr_mouse_button_value(
                    base_button,
                    false,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                let seq = crate::sgr_mouse_sequence(button_value, col, row, false);
                input.send(seq.as_bytes());
            }
        }

        fn on_mouse_move(
            &mut self,
            event: &MouseMoveEvent,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            if !event.modifiers.shift
                && self.input.is_some()
                && self.session.mouse_reporting_enabled()
                && self.session.mouse_sgr_enabled()
            {
                let send_motion = if self.session.mouse_any_event_enabled() {
                    true
                } else if self.session.mouse_button_event_enabled() {
                    event.pressed_button.is_some()
                } else {
                    false
                };

                if send_motion {
                    let Some((col, row)) = self.mouse_position_to_cell(event.position, window)
                    else {
                        return;
                    };

                    let base_button = match event.pressed_button {
                        Some(MouseButton::Left) => 0,
                        Some(MouseButton::Middle) => 1,
                        Some(MouseButton::Right) => 2,
                        Some(_) => 3,
                        None => 3,
                    };

                    let button_value = crate::sgr_mouse_button_value(
                        base_button,
                        true,
                        false,
                        event.modifiers.alt,
                        event.modifiers.control,
                    );
                    if let Some(input) = self.input.as_ref() {
                        let seq = crate::sgr_mouse_sequence(button_value, col, row, true);
                        input.send(seq.as_bytes());
                    }
                    return;
                }
            }

            if !event.dragging() {
                return;
            }

            if self.selection.is_none() {
                return;
            }

            let Some(index) = self.mouse_position_to_viewport_index(event.position, window) else {
                return;
            };

            if let Some(selection) = self.selection.as_mut() {
                if selection.active != index {
                    selection.active = index;
                    cx.notify();
                }
            }
        }

        fn on_key_down(
            &mut self,
            event: &KeyDownEvent,
            _window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            let keystroke = event.keystroke.clone().with_simulated_ime();

            if keystroke.modifiers.platform || keystroke.modifiers.function {
                return;
            }

            if keystroke.modifiers.control {
                let mut ctrl_byte: Option<u8> = None;

                if let Some(text) = keystroke.key_char.as_deref() {
                    let bytes = text.as_bytes();
                    if bytes.len() == 1 {
                        let b = bytes[0];
                        if (b'@'..=b'_').contains(&b) {
                            ctrl_byte = Some(b & 0x1f);
                        } else if (b'a'..=b'z').contains(&b) {
                            ctrl_byte = Some(b - b'a' + 1);
                        } else if (b'A'..=b'Z').contains(&b) {
                            ctrl_byte = Some(b - b'A' + 1);
                        }
                    }
                } else if keystroke.key == "space" {
                    ctrl_byte = Some(0x00);
                }

                if let Some(b) = ctrl_byte {
                    if let Some(input) = self.input.as_ref() {
                        input.send(&[b]);
                        return;
                    }

                    let _ = self.session.feed(&[b]);
                    self.apply_side_effects(cx);
                    self.schedule_viewport_refresh(cx);
                }
                return;
            }

            if keystroke.modifiers.alt {
                if let Some(text) = keystroke.key_char.as_deref() {
                    if let Some(input) = self.input.as_ref() {
                        input.send(&[0x1b]);
                        input.send(text.as_bytes());
                        return;
                    }
                }
                return;
            }

            let scroll_step = (self.session.rows() as i32 / 2).max(1);

            if let Some(input) = self.input.as_ref() {
                if !keystroke.modifiers.shift {
                    match keystroke.key.as_str() {
                        "home" => {
                            input.send(b"\x1b[H");
                            return;
                        }
                        "end" => {
                            input.send(b"\x1b[F");
                            return;
                        }
                        "pageup" | "page_up" | "page-up" => {
                            input.send(b"\x1b[5~");
                            return;
                        }
                        "pagedown" | "page_down" | "page-down" => {
                            input.send(b"\x1b[6~");
                            return;
                        }
                        _ => {}
                    }
                }

                match keystroke.key.as_str() {
                    "f1" => {
                        input.send(b"\x1bOP");
                        return;
                    }
                    "f2" => {
                        input.send(b"\x1bOQ");
                        return;
                    }
                    "f3" => {
                        input.send(b"\x1bOR");
                        return;
                    }
                    "f4" => {
                        input.send(b"\x1bOS");
                        return;
                    }
                    "f5" => {
                        input.send(b"\x1b[15~");
                        return;
                    }
                    "f6" => {
                        input.send(b"\x1b[17~");
                        return;
                    }
                    "f7" => {
                        input.send(b"\x1b[18~");
                        return;
                    }
                    "f8" => {
                        input.send(b"\x1b[19~");
                        return;
                    }
                    "f9" => {
                        input.send(b"\x1b[20~");
                        return;
                    }
                    "f10" => {
                        input.send(b"\x1b[21~");
                        return;
                    }
                    "f11" => {
                        input.send(b"\x1b[23~");
                        return;
                    }
                    "f12" => {
                        input.send(b"\x1b[24~");
                        return;
                    }
                    _ => {}
                }
            }

            match keystroke.key.as_str() {
                "home" => {
                    if self.input.is_some() && !keystroke.modifiers.shift {
                        return;
                    }
                    let _ = self.session.scroll_viewport_top();
                    self.apply_side_effects(cx);
                    self.schedule_viewport_refresh(cx);
                    return;
                }
                "end" => {
                    if self.input.is_some() && !keystroke.modifiers.shift {
                        return;
                    }
                    let _ = self.session.scroll_viewport_bottom();
                    self.apply_side_effects(cx);
                    self.schedule_viewport_refresh(cx);
                    return;
                }
                "pageup" | "page_up" | "page-up" => {
                    if self.input.is_some() && !keystroke.modifiers.shift {
                        return;
                    }
                    let _ = self.session.scroll_viewport(-scroll_step);
                    self.apply_side_effects(cx);
                    self.schedule_viewport_refresh(cx);
                    return;
                }
                "pagedown" | "page_down" | "page-down" => {
                    if self.input.is_some() && !keystroke.modifiers.shift {
                        return;
                    }
                    let _ = self.session.scroll_viewport(scroll_step);
                    self.apply_side_effects(cx);
                    self.schedule_viewport_refresh(cx);
                    return;
                }
                "up" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\x1b[A");
                        return;
                    }
                }
                "down" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\x1b[B");
                        return;
                    }
                }
                "right" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\x1b[C");
                        return;
                    }
                }
                "left" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\x1b[D");
                        return;
                    }
                }
                "escape" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(&[0x1b]);
                        return;
                    }
                }
                "delete" => {
                    if let Some(input) = self.input.as_ref() {
                        input.send(b"\x1b[3~");
                        return;
                    }
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
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }

            if keystroke.key == "backspace" {
                if let Some(input) = self.input.as_ref() {
                    input.send(&[0x7f]);
                    return;
                }
                let _ = self.session.feed(&[0x08]);
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
            }
        }

        fn on_scroll_wheel(
            &mut self,
            event: &ScrollWheelEvent,
            window: &mut Window,
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

            if let Some(input) = self.input.as_ref() {
                if !event.modifiers.shift
                    && self.session.mouse_reporting_enabled()
                    && self.session.mouse_sgr_enabled()
                {
                    let Some((col, row)) = self.mouse_position_to_cell(event.position, window)
                    else {
                        return;
                    };

                    let button = if delta_lines < 0 { 64 } else { 65 };
                    let button_value = crate::sgr_mouse_button_value(
                        button,
                        false,
                        false,
                        event.modifiers.alt,
                        event.modifiers.control,
                    );
                    let steps = delta_lines.unsigned_abs().min(10);
                    for _ in 0..steps {
                        let seq = crate::sgr_mouse_sequence(button_value, col, row, true);
                        input.send(seq.as_bytes());
                    }
                    return;
                }
            }

            let _ = self.session.scroll_viewport(delta_lines);
            self.apply_side_effects(cx);
            self.schedule_viewport_refresh(cx);
        }

        fn mouse_position_to_viewport_index(
            &self,
            position: gpui::Point<gpui::Pixels>,
            window: &mut Window,
        ) -> Option<usize> {
            let (col, row) = self.mouse_position_to_cell(position, window)?;
            Some(crate::viewport_index_for_cell(
                self.viewport.as_str(),
                row,
                col,
            ))
        }

        fn mouse_position_to_cell(
            &self,
            position: gpui::Point<gpui::Pixels>,
            window: &mut Window,
        ) -> Option<(u16, u16)> {
            let cols = self.session.cols();
            let rows = self.session.rows();

            let (cell_width, cell_height) = crate::cell_metrics(window)?;
            let x = f32::from(position.x);
            let y = f32::from(position.y);

            let mut col = (x / cell_width).floor() as i32 + 1;
            let mut row = (y / cell_height).floor() as i32 + 1;

            if col < 1 {
                col = 1;
            }
            if row < 1 {
                row = 1;
            }
            if col > cols as i32 {
                col = cols as i32;
            }
            if row > rows as i32 {
                row = rows as i32;
            }

            Some((col as u16, row as u16))
        }
    }

    impl Render for TerminalView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if !self.pending_output.is_empty() {
                let bytes = std::mem::take(&mut self.pending_output);
                let _ = self.session.feed(&bytes);
                self.apply_side_effects(cx);
                self.pending_refresh = true;
            }

            if self.pending_refresh {
                self.refresh_viewport();
                self.pending_refresh = false;
            }

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
                .on_action(cx.listener(Self::on_select_all))
                .on_action(cx.listener(Self::on_paste))
                .on_key_down(cx.listener(Self::on_key_down))
                .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                .on_mouse_move(cx.listener(Self::on_mouse_move))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
                .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
                .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
                .bg(gpui::black())
                .text_color(gpui::white())
                .font_family("monospace")
                .whitespace_nowrap()
                .child({
                    let highlight = HighlightStyle {
                        background_color: Some(gpui::hsla(0.58, 0.9, 0.55, 0.35)),
                        ..Default::default()
                    };

                    let mut text = StyledText::new(self.viewport.clone());
                    if let Some(selection) = self.selection {
                        let range = selection.range();
                        if !range.is_empty() {
                            text = text.with_highlights([(range, highlight)]);
                        }
                    }
                    text
                })
        }
    }
}

fn cell_metrics(window: &mut gpui::Window) -> Option<(f32, f32)> {
    let mut style = window.text_style();
    style.font_family = "monospace".into();

    let rem_size = window.rem_size();
    let font_size = style.font_size.to_pixels(rem_size);
    let line_height = style.line_height.to_pixels(style.font_size, rem_size);

    let run = style.to_run(1);
    let lines = window
        .text_system()
        .shape_text(
            gpui::SharedString::from("M"),
            font_size,
            &[run],
            None,
            Some(1),
        )
        .ok()?;
    let line = lines.first()?;

    let cell_width = f32::from(line.width()).max(1.0);
    let cell_height = f32::from(line_height).max(1.0);
    Some((cell_width, cell_height))
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

fn viewport_index_for_cell(viewport: &str, row: u16, col: u16) -> usize {
    let row = row.max(1) as usize;
    let col = col.max(1) as usize;

    let mut current_row = 1usize;
    let mut offset = 0usize;

    for segment in viewport.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);

        if current_row == row {
            if col == 1 {
                return offset;
            }

            let mut current_col = 1usize;
            for (byte_index, _) in line.char_indices() {
                if current_col == col {
                    return offset + byte_index;
                }
                current_col += 1;
            }

            return offset + line.len();
        }

        offset = offset.saturating_add(segment.len());
        current_row += 1;
    }

    viewport.len()
}

#[cfg(test)]
mod tests {
    use super::{TerminalConfig, TerminalSession};

    #[test]
    fn tracks_bracketed_paste_mode_from_output() {
        let mut session = TerminalSession::new(TerminalConfig::default()).unwrap();
        assert!(!session.bracketed_paste_enabled());

        session.feed(b"\x1b[?2004h").unwrap();
        assert!(session.bracketed_paste_enabled());

        session.feed(b"\x1b[?2004l").unwrap();
        assert!(!session.bracketed_paste_enabled());
    }

    #[test]
    fn tracks_mouse_reporting_mode_from_output() {
        let mut session = TerminalSession::new(TerminalConfig::default()).unwrap();
        assert!(!session.mouse_reporting_enabled());
        assert!(!session.mouse_sgr_enabled());

        session.feed(b"\x1b[?1000;1006h").unwrap();
        assert!(session.mouse_reporting_enabled());
        assert!(session.mouse_sgr_enabled());

        session.feed(b"\x1b[?1000l").unwrap();
        assert!(!session.mouse_reporting_enabled());
        assert!(session.mouse_sgr_enabled());

        session.feed(b"\x1b[?1006l").unwrap();
        assert!(!session.mouse_sgr_enabled());
    }

    #[test]
    fn viewport_index_maps_row_and_column_to_byte_index() {
        let viewport = "abc\ndef";

        assert_eq!(super::viewport_index_for_cell(viewport, 1, 1), 0);
        assert_eq!(super::viewport_index_for_cell(viewport, 1, 2), 1);
        assert_eq!(super::viewport_index_for_cell(viewport, 1, 4), 3);
        assert_eq!(super::viewport_index_for_cell(viewport, 1, 5), 3);

        assert_eq!(super::viewport_index_for_cell(viewport, 2, 1), 4);
        assert_eq!(super::viewport_index_for_cell(viewport, 2, 2), 5);
        assert_eq!(super::viewport_index_for_cell(viewport, 2, 4), 7);
        assert_eq!(super::viewport_index_for_cell(viewport, 2, 5), 7);

        assert_eq!(
            super::viewport_index_for_cell(viewport, 3, 1),
            viewport.len()
        );
    }

    #[test]
    fn update_viewport_string_skips_noop_updates() {
        let mut current = "abc".to_string();

        assert!(!super::update_viewport_string(
            &mut current,
            "abc".to_string()
        ));
        assert_eq!(current, "abc");

        assert!(super::update_viewport_string(
            &mut current,
            "def".to_string()
        ));
        assert_eq!(current, "def");
    }

    #[test]
    fn sgr_mouse_encoding_helpers_match_expected_format() {
        assert_eq!(
            super::sgr_mouse_button_value(0, false, false, false, false),
            0
        );
        assert_eq!(
            super::sgr_mouse_button_value(2, true, false, true, true),
            58
        );
        assert_eq!(super::sgr_mouse_sequence(0, 1, 1, true), "\u{1b}[<0;1;1M");
        assert_eq!(super::sgr_mouse_sequence(0, 1, 1, false), "\u{1b}[<0;1;1m");
    }
}

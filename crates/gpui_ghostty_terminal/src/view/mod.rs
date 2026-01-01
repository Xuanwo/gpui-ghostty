
use super::TerminalSession;
use ghostty_vt::{KeyModifiers, Rgb, StyleRun, encode_key_named};
use gpui::{
    App, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler,
    EntityInputHandler, FocusHandle, GlobalElementId, IntoElement, KeyDownEvent, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Render,
    ScrollDelta, ScrollWheelEvent, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle,
    Window, actions, div, fill, hsla, point, prelude::*, px, relative, rgba, size,
};
use std::ops::Range;

actions!(terminal_view, [Copy, Paste, SelectAll]);

fn split_viewport_lines(viewport: &str) -> Vec<String> {
    let viewport = viewport.strip_suffix('\n').unwrap_or(viewport);
    if viewport.is_empty() {
        return Vec::new();
    }
    viewport.split('\n').map(|line| line.to_string()).collect()
}

pub(crate) fn should_skip_key_down_for_ime(has_input: bool, keystroke: &gpui::Keystroke) -> bool {
    if !has_input || !keystroke.is_ime_in_progress() {
        return false;
    }

    !matches!(
        keystroke.key.as_str(),
        "enter" | "return" | "kp_enter" | "numpad_enter"
    )
}

pub(crate) fn ctrl_byte_for_keystroke(keystroke: &gpui::Keystroke) -> Option<u8> {
    let candidate = keystroke
        .key_char
        .as_deref()
        .or_else(|| (!keystroke.key.is_empty()).then_some(keystroke.key.as_str()))?;

    if candidate == "space" {
        return Some(0x00);
    }

    let bytes = candidate.as_bytes();
    if bytes.len() != 1 {
        return None;
    }

    let b = bytes[0];
    if (b'@'..=b'_').contains(&b) {
        Some(b & 0x1f)
    } else if (b'a'..=b'z').contains(&b) {
        Some(b - b'a' + 1)
    } else if (b'A'..=b'Z').contains(&b) {
        Some(b - b'A' + 1)
    } else {
        None
    }
}

pub(crate) fn sgr_mouse_button_value(
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

pub(crate) fn sgr_mouse_sequence(button_value: u8, col: u16, row: u16, pressed: bool) -> String {
    let suffix = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", button_value, col, row, suffix)
}

fn is_url_byte(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b':'
                | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'%'
        )
}

fn url_at_byte_index(text: &str, index: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut idx = index.min(bytes.len().saturating_sub(1));

    if !is_url_byte(bytes[idx]) && idx > 0 && is_url_byte(bytes[idx - 1]) {
        idx -= 1;
    }

    if !is_url_byte(bytes[idx]) {
        return None;
    }

    let mut start = idx;
    while start > 0 && is_url_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = idx + 1;
    while end < bytes.len() && is_url_byte(bytes[end]) {
        end += 1;
    }

    while end > start
        && matches!(
            bytes[end - 1],
            b'.' | b',' | b')' | b']' | b'}' | b';' | b':' | b'!' | b'?'
        )
    {
        end -= 1;
    }

    let candidate = std::str::from_utf8(&bytes[start..end]).ok()?;
    if candidate.starts_with("https://") || candidate.starts_with("http://") {
        Some(candidate.to_string())
    } else {
        None
    }
}

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
    viewport_lines: Vec<String>,
    viewport_line_offsets: Vec<usize>,
    viewport_total_len: usize,
    viewport_style_runs: Vec<Vec<StyleRun>>,
    line_layouts: Vec<Option<gpui::ShapedLine>>,
    line_layout_key: Option<(Pixels, Pixels)>,
    focus_handle: FocusHandle,
    last_window_title: Option<String>,
    input: Option<TerminalInput>,
    pending_output: Vec<u8>,
    pending_refresh: bool,
    selection: Option<ByteSelection>,
    marked_text: Option<SharedString>,
    marked_selected_range_utf16: Range<usize>,
    font: gpui::Font,
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
            viewport_lines: Vec::new(),
            viewport_line_offsets: Vec::new(),
            viewport_total_len: 0,
            viewport_style_runs: Vec::new(),
            line_layouts: Vec::new(),
            line_layout_key: None,
            focus_handle,
            last_window_title: None,
            input: None,
            pending_output: Vec::new(),
            pending_refresh: false,
            selection: None,
            marked_text: None,
            marked_selected_range_utf16: 0..0,
            font: crate::default_terminal_font(),
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
            viewport_lines: Vec::new(),
            viewport_line_offsets: Vec::new(),
            viewport_total_len: 0,
            viewport_style_runs: Vec::new(),
            line_layouts: Vec::new(),
            line_layout_key: None,
            focus_handle,
            last_window_title: None,
            input: Some(input),
            pending_output: Vec::new(),
            pending_refresh: false,
            selection: None,
            marked_text: None,
            marked_selected_range_utf16: 0..0,
            font: crate::default_terminal_font(),
        }
        .with_refreshed_viewport()
    }

    fn utf16_len(s: &str) -> usize {
        s.chars().map(|ch| ch.len_utf16()).sum()
    }

    fn utf16_range_to_utf8(s: &str, range_utf16: Range<usize>) -> Option<Range<usize>> {
        let mut utf16_count = 0usize;
        let mut start_utf8: Option<usize> = None;
        let mut end_utf8: Option<usize> = None;

        if range_utf16.start == 0 {
            start_utf8 = Some(0);
        }
        if range_utf16.end == 0 {
            end_utf8 = Some(0);
        }

        for (utf8_index, ch) in s.char_indices() {
            if start_utf8.is_none() && utf16_count >= range_utf16.start {
                start_utf8 = Some(utf8_index);
            }
            if end_utf8.is_none() && utf16_count >= range_utf16.end {
                end_utf8 = Some(utf8_index);
            }

            utf16_count = utf16_count.saturating_add(ch.len_utf16());
        }

        if start_utf8.is_none() && utf16_count >= range_utf16.start {
            start_utf8 = Some(s.len());
        }
        if end_utf8.is_none() && utf16_count >= range_utf16.end {
            end_utf8 = Some(s.len());
        }

        Some(start_utf8?..end_utf8?)
    }

    fn cell_offset_for_utf16(text: &str, utf16_offset: usize) -> usize {
        use unicode_width::UnicodeWidthChar as _;

        let mut cells = 0usize;
        let mut utf16_count = 0usize;
        for ch in text.chars() {
            if utf16_count >= utf16_offset {
                break;
            }

            let len_utf16 = ch.len_utf16();
            if utf16_count.saturating_add(len_utf16) > utf16_offset {
                break;
            }
            utf16_count = utf16_count.saturating_add(len_utf16);

            let width = ch.width().unwrap_or(0);
            if width > 0 {
                cells = cells.saturating_add(width);
            }
        }
        cells
    }

    fn clear_marked_text(&mut self, cx: &mut Context<Self>) {
        self.marked_text = None;
        self.marked_selected_range_utf16 = 0..0;
        cx.notify();
    }

    fn set_marked_text(
        &mut self,
        text: String,
        selected_range_utf16: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            self.clear_marked_text(cx);
            return;
        }

        let total_utf16 = Self::utf16_len(&text);
        let selected = selected_range_utf16.unwrap_or(total_utf16..total_utf16);
        let selected = selected.start.min(total_utf16)..selected.end.min(total_utf16);

        self.marked_text = Some(SharedString::from(text));
        self.marked_selected_range_utf16 = selected;
        cx.notify();
    }

    fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }

        if let Some(input) = self.input.as_ref() {
            input.send(text.as_bytes());
            return;
        }

        let _ = self.session.feed(text.as_bytes());
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }

    fn with_refreshed_viewport(mut self) -> Self {
        self.refresh_viewport();
        self
    }

    fn refresh_viewport(&mut self) {
        let viewport = self.session.dump_viewport().unwrap_or_default();
        self.viewport_lines = split_viewport_lines(&viewport);
        self.viewport_line_offsets = Self::compute_viewport_line_offsets(&self.viewport_lines);
        self.viewport_total_len = Self::compute_viewport_total_len(&self.viewport_lines);
        self.viewport_style_runs = (0..self.session.rows())
            .map(|row| {
                self.session
                    .dump_viewport_row_style_runs(row)
                    .unwrap_or_default()
            })
            .collect();
        self.line_layouts.clear();
        self.line_layout_key = None;
        self.selection = None;
    }

    fn compute_viewport_line_offsets(lines: &[String]) -> Vec<usize> {
        let mut offsets = Vec::with_capacity(lines.len());
        let mut offset = 0usize;
        for line in lines {
            offsets.push(offset);
            offset = offset.saturating_add(line.len() + 1);
        }
        offsets
    }

    fn compute_viewport_total_len(lines: &[String]) -> usize {
        lines
            .iter()
            .fold(0usize, |acc, line| acc.saturating_add(line.len() + 1))
    }

    fn viewport_slice(&self, range: Range<usize>) -> String {
        if range.is_empty() || self.viewport_lines.is_empty() {
            return String::new();
        }

        let start = range.start.min(self.viewport_total_len);
        let end = range.end.min(self.viewport_total_len);
        if start >= end {
            return String::new();
        }

        let mut out = String::new();
        let mut i = 0usize;
        while i < self.viewport_lines.len() {
            let line_start = *self.viewport_line_offsets.get(i).unwrap_or(&0);
            let line = &self.viewport_lines[i];
            let line_end = line_start.saturating_add(line.len());
            let newline_pos = line_end;

            let seg_start = start.max(line_start);
            let seg_end = end.min(newline_pos.saturating_add(1));
            if seg_start < seg_end {
                let local_start = seg_start.saturating_sub(line_start);
                let local_end = seg_end.saturating_sub(line_start);
                let local_end = local_end.min(line.len().saturating_add(1));

                if local_start < line.len() {
                    let text_end = local_end.min(line.len());
                    if let Some(seg) = line.get(local_start..text_end) {
                        out.push_str(seg);
                    }
                }
                if local_end > line.len() {
                    out.push('\n');
                }
            }

            i += 1;
        }

        out
    }

    fn url_at_viewport_index(&self, index: usize) -> Option<String> {
        if self.viewport_lines.is_empty() {
            return None;
        }

        let idx = index.min(self.viewport_total_len.saturating_sub(1));
        let row = self
            .viewport_line_offsets
            .iter()
            .enumerate()
            .rfind(|(_, offset)| **offset <= idx)
            .map(|(i, _)| i)?;

        let line = self.viewport_lines.get(row)?.as_str();
        let line_start = *self.viewport_line_offsets.get(row).unwrap_or(&0);
        let local = idx
            .saturating_sub(line_start)
            .min(line.len().saturating_sub(1));
        url_at_byte_index(line, local)
    }

    fn apply_dirty_viewport_rows(&mut self, dirty_rows: &[u16]) -> bool {
        if dirty_rows.is_empty() {
            return false;
        }

        let expected_rows = self.session.rows() as usize;
        if self.viewport_lines.len() != expected_rows {
            self.refresh_viewport();
            return true;
        }
        if self.viewport_style_runs.len() != expected_rows {
            self.refresh_viewport();
            return true;
        }

        for &row in dirty_rows {
            let row = row as usize;
            if row >= self.viewport_lines.len() {
                continue;
            }

            let line = match self.session.dump_viewport_row(row as u16) {
                Ok(s) => s,
                Err(_) => {
                    self.refresh_viewport();
                    return true;
                }
            };

            let line = line.strip_suffix('\n').unwrap_or(line.as_str());
            self.viewport_lines[row].clear();
            self.viewport_lines[row].push_str(line);
            self.viewport_style_runs[row] = self
                .session
                .dump_viewport_row_style_runs(row as u16)
                .unwrap_or_default();
            if row < self.line_layouts.len() {
                self.line_layouts[row] = None;
            }
        }

        self.viewport_line_offsets = Self::compute_viewport_line_offsets(&self.viewport_lines);
        self.viewport_total_len = Self::compute_viewport_total_len(&self.viewport_lines);
        self.selection = None;
        true
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
        if let Some(input) = self.input.as_ref() {
            let _ = self
                .session
                .feed_with_pty_responses(bytes, |resp| input.send(resp));
        } else {
            let _ = self.session.feed(bytes);
        }
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
            if let Some(input) = self.input.as_ref() {
                let _ = self
                    .session
                    .feed_with_pty_responses(&pending, |resp| input.send(resp));
            } else {
                let _ = self.session.feed(&pending);
            }
            self.apply_side_effects(cx);
            let dirty = self.session.take_dirty_viewport_rows();
            if !dirty.is_empty() && !self.apply_dirty_viewport_rows(&dirty) {
                self.pending_refresh = true;
            }
        }

        if bytes.len() > MAX_PENDING_OUTPUT_BYTES {
            let mut offset = 0usize;
            while offset < bytes.len() {
                let end = (offset + MAX_PENDING_OUTPUT_BYTES).min(bytes.len());
                if let Some(input) = self.input.as_ref() {
                    let _ = self
                        .session
                        .feed_with_pty_responses(&bytes[offset..end], |resp| input.send(resp));
                } else {
                    let _ = self.session.feed(&bytes[offset..end]);
                }
                offset = end;
            }
            self.apply_side_effects(cx);
            let dirty = self.session.take_dirty_viewport_rows();
            if !dirty.is_empty() && !self.apply_dirty_viewport_rows(&dirty) {
                self.pending_refresh = true;
            }
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
            .map(|range| self.viewport_slice(range))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.viewport_slice(0..self.viewport_total_len));

        let item = ClipboardItem::new_string(selection.to_string());
        cx.write_to_clipboard(item.clone());
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        cx.write_to_primary(item);
    }

    fn on_select_all(&mut self, _: &SelectAll, window: &mut Window, cx: &mut Context<Self>) {
        self.selection = Some(ByteSelection {
            anchor: 0,
            active: self.viewport_total_len,
        });
        self.on_copy(&Copy, window, cx);
        cx.notify();
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

        if event.button == MouseButton::Left && event.modifiers.platform {
            if let Some((col, row)) = self.mouse_position_to_cell(event.position, window) {
                if let Some(link) = self.session.hyperlink_at(col, row) {
                    let item = ClipboardItem::new_string(link);
                    cx.write_to_clipboard(item.clone());
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    cx.write_to_primary(item);
                    return;
                }
            }

            if let Some(index) = self.mouse_position_to_viewport_index(event.position, window)
                && let Some(url) = self.url_at_viewport_index(index)
            {
                let item = ClipboardItem::new_string(url);
                cx.write_to_clipboard(item.clone());
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                cx.write_to_primary(item);
                return;
            }
        }

        if event.modifiers.shift
            || self.input.is_none()
            || !self.session.mouse_reporting_enabled()
            || !self.session.mouse_sgr_enabled()
        {
            if event.button == MouseButton::Left {
                if let Some(index) = self.mouse_position_to_viewport_index(event.position, window) {
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

            let button_value = sgr_mouse_button_value(
                base_button,
                false,
                false,
                event.modifiers.alt,
                event.modifiers.control,
            );
            let seq = sgr_mouse_sequence(button_value, col, row, true);
            input.send(seq.as_bytes());
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
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

            let button_value = sgr_mouse_button_value(
                base_button,
                false,
                false,
                event.modifiers.alt,
                event.modifiers.control,
            );
            let seq = sgr_mouse_sequence(button_value, col, row, false);
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
                let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                    return;
                };

                let base_button = match event.pressed_button {
                    Some(MouseButton::Left) => 0,
                    Some(MouseButton::Middle) => 1,
                    Some(MouseButton::Right) => 2,
                    Some(_) => 3,
                    None => 3,
                };

                let button_value = sgr_mouse_button_value(
                    base_button,
                    true,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                if let Some(input) = self.input.as_ref() {
                    let seq = sgr_mouse_sequence(button_value, col, row, true);
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

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let raw_keystroke = event.keystroke.clone();
        if should_skip_key_down_for_ime(self.input.is_some(), &raw_keystroke) {
            return;
        }
        let keystroke = raw_keystroke.with_simulated_ime();

        if keystroke.modifiers.platform || keystroke.modifiers.function {
            return;
        }

        let scroll_step = (self.session.rows() as i32 / 2).max(1);

        if let Some(input) = self.input.as_ref() {
            if keystroke.modifiers.shift {
                match keystroke.key.as_str() {
                    "home" => {
                        let _ = self.session.scroll_viewport_top();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "end" => {
                        let _ = self.session.scroll_viewport_bottom();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "pageup" | "page_up" | "page-up" => {
                        let _ = self.session.scroll_viewport(-scroll_step);
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "pagedown" | "page_down" | "page-down" => {
                        let _ = self.session.scroll_viewport(scroll_step);
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    _ => {}
                }
            }

            if keystroke.modifiers.control {
                if let Some(b) = ctrl_byte_for_keystroke(&keystroke) {
                    input.send(&[b]);
                    return;
                }
            }

            if keystroke.modifiers.alt {
                if let Some(text) = keystroke.key_char.as_deref() {
                    input.send(&[0x1b]);
                    input.send(text.as_bytes());
                    return;
                }
            }

            let modifiers = KeyModifiers {
                shift: keystroke.modifiers.shift,
                control: keystroke.modifiers.control,
                alt: keystroke.modifiers.alt,
                super_key: false,
            };
            if let Some(encoded) = encode_key_named(&keystroke.key, modifiers) {
                input.send(&encoded);
                return;
            }
            return;
        }

        match keystroke.key.as_str() {
            "home" => {
                let _ = self.session.scroll_viewport_top();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "end" => {
                let _ = self.session.scroll_viewport_bottom();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "pageup" | "page_up" | "page-up" => {
                let _ = self.session.scroll_viewport(-scroll_step);
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "pagedown" | "page_down" | "page-down" => {
                let _ = self.session.scroll_viewport(scroll_step);
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            _ => {}
        }

        let modifiers = KeyModifiers {
            shift: keystroke.modifiers.shift,
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            super_key: false,
        };
        if let Some(encoded) = encode_key_named(&keystroke.key, modifiers) {
            let _ = self.session.feed(&encoded);
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
                let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                    return;
                };

                let button = if delta_lines < 0 { 64 } else { 65 };
                let button_value = sgr_mouse_button_value(
                    button,
                    false,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                let steps = delta_lines.unsigned_abs().min(10);
                for _ in 0..steps {
                    let seq = sgr_mouse_sequence(button_value, col, row, true);
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
        let rows = self.session.rows() as usize;
        if rows == 0 {
            return None;
        }

        let (_, cell_height) = cell_metrics(window, &self.font)?;
        let y = f32::from(position.y);
        let mut row_index = (y / cell_height).floor() as i32;
        if row_index < 0 {
            row_index = 0;
        }
        if row_index >= rows as i32 {
            row_index = rows as i32 - 1;
        }
        let row_index = row_index as usize;

        if let Some(Some(line)) = self.line_layouts.get(row_index) {
            let byte_index = line
                .closest_index_for_x(px(f32::from(position.x)))
                .min(line.text.len());
            let offset = *self.viewport_line_offsets.get(row_index).unwrap_or(&0);
            return Some(offset.saturating_add(byte_index));
        }

        let (col, row) = self.mouse_position_to_cell(position, window)?;
        let row_index = row.saturating_sub(1) as usize;
        let line = self.viewport_lines.get(row_index)?.as_str();
        let byte_index = byte_index_for_column_in_line(line, col).min(line.len());
        let offset = *self.viewport_line_offsets.get(row_index).unwrap_or(&0);
        Some(offset.saturating_add(byte_index))
    }

    fn mouse_position_to_cell(
        &self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
    ) -> Option<(u16, u16)> {
        let cols = self.session.cols();
        let rows = self.session.rows();

        let (cell_width, cell_height) = cell_metrics(window, &self.font)?;
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

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.marked_text.as_ref()?.as_str();
        let total_utf16 = Self::utf16_len(text);
        let start = range_utf16.start.min(total_utf16);
        let end = range_utf16.end.min(total_utf16);
        let range_utf16 = start..end;
        *adjusted_range = Some(range_utf16.clone());

        let range_utf8 = Self::utf16_range_to_utf8(text, range_utf16)?;
        Some(text.get(range_utf8)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.marked_selected_range_utf16.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.marked_text.as_ref()?.as_str();
        let len = Self::utf16_len(text);
        (len > 0).then_some(0..len)
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.clear_marked_text(cx);
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_marked_text(cx);
        self.commit_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_marked_text(new_text.to_string(), new_selected_range, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (col, row) = self.session.cursor_position()?;
        let (cell_width, cell_height) = cell_metrics(window, &self.font)?;

        let base_x = element_bounds.left() + px(cell_width * (col.saturating_sub(1)) as f32);
        let base_y = element_bounds.top() + px(cell_height * (row.saturating_sub(1)) as f32);

        let offset_cells = self
            .marked_text
            .as_ref()
            .map(|text| Self::cell_offset_for_utf16(text.as_str(), range_utf16.start))
            .unwrap_or(range_utf16.start);
        let x = base_x + px(cell_width * offset_cells as f32);
        Some(Bounds::new(
            point(x, base_y),
            size(px(cell_width), px(cell_height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

struct TerminalPrepaintState {
    line_height: Pixels,
    shaped_lines: Vec<gpui::ShapedLine>,
    background_quads: Vec<PaintQuad>,
    selection_quads: Vec<PaintQuad>,
    box_drawing_quads: Vec<PaintQuad>,
    marked_text: Option<(gpui::ShapedLine, gpui::Point<Pixels>)>,
    marked_text_background: Option<PaintQuad>,
    cursor: Option<PaintQuad>,
}

const CELL_STYLE_FLAG_BOLD: u8 = 0x02;
const CELL_STYLE_FLAG_ITALIC: u8 = 0x04;
const CELL_STYLE_FLAG_UNDERLINE: u8 = 0x08;
const CELL_STYLE_FLAG_FAINT: u8 = 0x10;
const CELL_STYLE_FLAG_STRIKETHROUGH: u8 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextRunKey {
    fg: Rgb,
    flags: u8,
}

fn hsla_from_rgb(rgb: Rgb) -> gpui::Hsla {
    let rgba = gpui::Rgba {
        r: rgb.r as f32 / 255.0,
        g: rgb.g as f32 / 255.0,
        b: rgb.b as f32 / 255.0,
        a: 1.0,
    };
    rgba.into()
}

fn font_for_flags(base: &gpui::Font, flags: u8) -> gpui::Font {
    let mut font = base.clone();
    if flags & CELL_STYLE_FLAG_BOLD != 0 {
        font = font.bold();
    }
    if flags & CELL_STYLE_FLAG_ITALIC != 0 {
        font = font.italic();
    }
    font
}

fn color_for_key(key: TextRunKey) -> gpui::Hsla {
    let mut color = hsla_from_rgb(key.fg);
    if key.flags & CELL_STYLE_FLAG_FAINT != 0 {
        color = color.alpha(0.65);
    }
    color
}

pub(crate) const BOX_DIR_LEFT: u8 = 0x01;
pub(crate) const BOX_DIR_RIGHT: u8 = 0x02;
pub(crate) const BOX_DIR_UP: u8 = 0x04;
pub(crate) const BOX_DIR_DOWN: u8 = 0x08;

pub(crate) fn box_drawing_mask(ch: char) -> Option<(u8, f32)> {
    let light = 1.0;
    let heavy = 1.35;
    let double = 1.15;

    let mask = match ch {
        '─' | '━' | '═' => BOX_DIR_LEFT | BOX_DIR_RIGHT,
        '│' | '┃' | '║' => BOX_DIR_UP | BOX_DIR_DOWN,
        '┌' | '┏' | '╔' | '╭' => BOX_DIR_RIGHT | BOX_DIR_DOWN,
        '┐' | '┓' | '╗' | '╮' => BOX_DIR_LEFT | BOX_DIR_DOWN,
        '└' | '┗' | '╚' | '╰' => BOX_DIR_RIGHT | BOX_DIR_UP,
        '┘' | '┛' | '╝' | '╯' => BOX_DIR_LEFT | BOX_DIR_UP,
        '├' | '┣' | '╠' => BOX_DIR_RIGHT | BOX_DIR_UP | BOX_DIR_DOWN,
        '┤' | '┫' | '╣' => BOX_DIR_LEFT | BOX_DIR_UP | BOX_DIR_DOWN,
        '┬' | '┳' | '╦' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_DOWN,
        '┴' | '┻' | '╩' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_UP,
        '┼' | '╋' | '╬' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_UP | BOX_DIR_DOWN,
        _ => return None,
    };

    let scale = match ch {
        '━' | '┃' | '┏' | '┓' | '┗' | '┛' | '┣' | '┫' | '┳' | '┻' | '╋' => {
            heavy
        }
        '═' | '║' | '╔' | '╗' | '╚' | '╝' | '╠' | '╣' | '╦' | '╩' | '╬' => {
            double
        }
        _ => light,
    };

    Some((mask, scale))
}

fn box_drawing_quads_for_char(
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    cell_width: f32,
    color: gpui::Hsla,
    ch: char,
) -> Vec<PaintQuad> {
    let Some((mask, scale)) = box_drawing_mask(ch) else {
        return Vec::new();
    };

    let x0 = bounds.left();
    let x1 = x0 + px(cell_width);
    let y0 = bounds.top();
    let y1 = y0 + line_height;

    let mid_x = x0 + px(cell_width * 0.5);
    let mid_y = y0 + line_height * 0.5;

    let thickness = px(((f32::from(line_height) / 12.0).max(1.0) * scale).max(1.0));
    let half_t = thickness * 0.5;

    let has_left = mask & BOX_DIR_LEFT != 0;
    let has_right = mask & BOX_DIR_RIGHT != 0;
    let has_up = mask & BOX_DIR_UP != 0;
    let has_down = mask & BOX_DIR_DOWN != 0;

    let mut quads = Vec::new();

    if has_left || has_right {
        let (start_x, end_x) = if has_left && has_right {
            (x0, x1)
        } else if has_left {
            (x0, mid_x)
        } else {
            (mid_x, x1)
        };
        quads.push(fill(
            Bounds::from_corners(point(start_x, mid_y - half_t), point(end_x, mid_y + half_t)),
            color,
        ));
    }

    if has_up || has_down {
        let (start_y, end_y) = if has_up && has_down {
            (y0, y1)
        } else if has_up {
            (y0, mid_y)
        } else {
            (mid_y, y1)
        };

        quads.push(fill(
            Bounds::from_corners(point(mid_x - half_t, start_y), point(mid_x + half_t, end_y)),
            color,
        ));
    }

    quads
}

fn text_run_for_key(base_font: &gpui::Font, key: TextRunKey, len: usize) -> TextRun {
    let font = font_for_flags(base_font, key.flags);
    let color = color_for_key(key);

    let underline = (key.flags & CELL_STYLE_FLAG_UNDERLINE != 0).then_some(UnderlineStyle {
        color: Some(color),
        thickness: px(1.0),
        wavy: false,
    });

    let strikethrough =
        (key.flags & CELL_STYLE_FLAG_STRIKETHROUGH != 0).then_some(gpui::StrikethroughStyle {
            color: Some(color),
            thickness: px(1.0),
        });

    TextRun {
        len,
        font,
        color,
        background_color: None,
        underline,
        strikethrough,
    }
}

pub(crate) fn byte_index_for_column_in_line(line: &str, col: u16) -> usize {
    use unicode_width::UnicodeWidthChar as _;

    let col = col.max(1) as usize;
    if col == 1 {
        return 0;
    }

    let mut current_col = 1usize;
    for (byte_index, ch) in line.char_indices() {
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            continue;
        }

        if current_col == col {
            return byte_index;
        }

        let next_col = current_col.saturating_add(width);
        if col < next_col {
            return byte_index;
        }

        current_col = next_col;
    }

    line.len()
}

struct TerminalTextElement {
    view: gpui::Entity<TerminalView>,
}

impl IntoElement for TerminalTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalTextElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let mut style = window.text_style();
        let font = { self.view.read(cx).font.clone() };
        style.font_family = font.family.clone();
        style.font_features = crate::default_terminal_font_features();
        style.font_fallbacks = font.fallbacks.clone();
        let default_fg = { self.view.read(cx).session.default_foreground() };
        style.color = hsla_from_rgb(default_fg);
        let rem_size = window.rem_size();
        let font_size = style.font_size.to_pixels(rem_size);
        let line_height = style.line_height.to_pixels(style.font_size, rem_size);

        let run_font = style.font();
        let run_color = style.color;

        let cell_width = cell_metrics(window, &font).map(|(w, _)| px(w));

        self.view.update(cx, |view, _cx| {
            if view.viewport_lines.is_empty() {
                view.line_layouts.clear();
                view.line_layout_key = None;
                return;
            }

            if view.line_layout_key != Some((font_size, line_height))
                || view.line_layouts.len() != view.viewport_lines.len()
            {
                view.line_layout_key = Some((font_size, line_height));
                view.line_layouts = vec![None; view.viewport_lines.len()];
            }

            for (idx, line) in view.viewport_lines.iter().enumerate() {
                let Some(slot) = view.line_layouts.get_mut(idx) else {
                    continue;
                };

                if let Some(existing) = slot.as_ref()
                    && existing.text.as_str() == line.as_str()
                {
                    continue;
                }

                let text = SharedString::from(line.clone());
                let mut runs: Vec<TextRun> = Vec::new();

                if let Some(style_runs) = view.viewport_style_runs.get(idx)
                    && !style_runs.is_empty()
                {
                    let mut byte_pos = 0usize;
                    for style in style_runs.iter() {
                        let key = TextRunKey {
                            fg: style.fg,
                            flags: style.flags
                                & (CELL_STYLE_FLAG_BOLD
                                    | CELL_STYLE_FLAG_ITALIC
                                    | CELL_STYLE_FLAG_UNDERLINE
                                    | CELL_STYLE_FLAG_FAINT
                                    | CELL_STYLE_FLAG_STRIKETHROUGH),
                        };

                        let start = byte_index_for_column_in_line(text.as_str(), style.start_col)
                            .min(text.len());
                        let end = byte_index_for_column_in_line(
                            text.as_str(),
                            style.end_col.saturating_add(1),
                        )
                        .min(text.len());

                        if start > byte_pos {
                            runs.push(TextRun {
                                len: start.saturating_sub(byte_pos),
                                font: run_font.clone(),
                                color: run_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                            byte_pos = start;
                        }

                        if end > start {
                            runs.push(text_run_for_key(&run_font, key, end.saturating_sub(start)));
                            byte_pos = end;
                        }
                    }

                    if byte_pos < text.len() {
                        runs.push(TextRun {
                            len: text.len().saturating_sub(byte_pos),
                            font: run_font.clone(),
                            color: run_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }
                }

                if runs.is_empty() {
                    runs.push(TextRun {
                        len: text.len(),
                        font: run_font.clone(),
                        color: run_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                }

                let force_width = cell_width.and_then(|cell_width| {
                    use unicode_width::UnicodeWidthChar as _;
                    let has_wide = text.as_str().chars().any(|ch| ch.width().unwrap_or(0) > 1);
                    (!has_wide).then_some(cell_width)
                });
                let shaped = window
                    .text_system()
                    .shape_line(text, font_size, &runs, force_width);
                *slot = Some(shaped);
            }
        });

        let default_bg = { self.view.read(cx).session.default_background() };
        let background_quads = cell_metrics(window, &font)
            .map(|(cell_width, _)| {
                let origin = bounds.origin;
                let mut quads: Vec<PaintQuad> = Vec::new();

                let view = self.view.read(cx);
                for (row, runs) in view.viewport_style_runs.iter().enumerate() {
                    if runs.is_empty() {
                        continue;
                    }

                    let y = origin.y + line_height * row as f32;
                    for run in runs.iter() {
                        if run.bg == default_bg {
                            continue;
                        }

                        let x =
                            origin.x + px(cell_width * (run.start_col.saturating_sub(1)) as f32);
                        let w = px(cell_width
                            * (run.end_col.saturating_sub(run.start_col).saturating_add(1)) as f32);
                        let color = rgba(
                            (u32::from(run.bg.r) << 24)
                                | (u32::from(run.bg.g) << 16)
                                | (u32::from(run.bg.b) << 8)
                                | 0xFF,
                        );
                        quads.push(fill(Bounds::new(point(x, y), size(w, line_height)), color));
                    }
                }

                quads
            })
            .unwrap_or_default();

        let (shaped_lines, selection, line_offsets) = {
            let view = self.view.read(cx);
            (
                view.line_layouts
                    .iter()
                    .map(|line| line.clone().unwrap_or_default())
                    .collect::<Vec<_>>(),
                view.selection,
                view.viewport_line_offsets.clone(),
            )
        };

        let (marked_text, cursor_position, font) = {
            let view = self.view.read(cx);
            (
                view.marked_text.clone(),
                view.session.cursor_position(),
                view.font.clone(),
            )
        };

        let (marked_text, marked_text_background) = marked_text
            .and_then(|text| {
                if text.is_empty() {
                    return None;
                }
                let (col, row) = cursor_position?;
                let (cell_width, _) = cell_metrics(window, &font)?;

                let origin_x = bounds.left() + px(cell_width * (col.saturating_sub(1)) as f32);
                let origin_y = bounds.top() + line_height * (row.saturating_sub(1)) as f32;
                let origin = point(origin_x, origin_y);

                let run = TextRun {
                    len: text.len(),
                    font: run_font.clone(),
                    color: run_color,
                    background_color: None,
                    underline: Some(UnderlineStyle {
                        color: Some(run_color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let force_width = {
                    use unicode_width::UnicodeWidthChar as _;
                    let has_wide = text.as_str().chars().any(|ch| ch.width().unwrap_or(0) > 1);
                    (!has_wide).then_some(px(cell_width))
                };
                let shaped =
                    window
                        .text_system()
                        .shape_line(text.clone(), font_size, &[run], force_width);

                let bg = {
                    let view = self.view.read(cx);
                    let row_index = row.saturating_sub(1) as usize;
                    view.viewport_style_runs
                        .get(row_index)
                        .and_then(|runs| {
                            runs.iter().find_map(|run| {
                                (col >= run.start_col && col <= run.end_col).then_some(run.bg)
                            })
                        })
                        .unwrap_or(default_bg)
                };

                let cell_len = {
                    use unicode_width::UnicodeWidthChar as _;
                    let mut cells = 0usize;
                    for ch in text.as_str().chars() {
                        let w = ch.width().unwrap_or(0);
                        if w > 0 {
                            cells = cells.saturating_add(w);
                        }
                    }
                    cells.max(1)
                };

                let marked_text_background = fill(
                    Bounds::new(origin, size(px(cell_width * cell_len as f32), line_height)),
                    rgba(
                        (u32::from(bg.r) << 24)
                            | (u32::from(bg.g) << 16)
                            | (u32::from(bg.b) << 8)
                            | 0xFF,
                    ),
                );

                Some(((shaped, origin), marked_text_background))
            })
            .map(|(text, bg)| (Some(text), Some(bg)))
            .unwrap_or((None, None));

        let selection_quads = selection
            .map(|sel| sel.range())
            .filter(|range| !range.is_empty())
            .map(|range| {
                let highlight = hsla(0.58, 0.9, 0.55, 0.35);
                let mut quads = Vec::new();

                for (row, line) in shaped_lines.iter().enumerate() {
                    let Some(&line_offset) = line_offsets.get(row) else {
                        continue;
                    };

                    let line_start = line_offset;
                    let line_end = line_offset.saturating_add(line.text.len());

                    let seg_start = range.start.max(line_start).min(line_end);
                    let seg_end = range.end.max(line_start).min(line_end);
                    if seg_start >= seg_end {
                        continue;
                    }

                    let local_start = seg_start.saturating_sub(line_start);
                    let local_end = seg_end.saturating_sub(line_start);

                    let x1 = line.x_for_index(local_start);
                    let x2 = line.x_for_index(local_end);

                    let y1 = bounds.top() + line_height * row as f32;
                    let y2 = y1 + line_height;

                    quads.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + x1, y1),
                            point(bounds.left() + x2, y2),
                        ),
                        highlight,
                    ));
                }

                quads
            })
            .unwrap_or_default();

        let box_drawing_quads = cell_metrics(window, &font)
            .map(|(cell_width, _)| {
                use unicode_width::UnicodeWidthChar as _;
                let default_fg = run_color;
                let mut quads = Vec::new();

                let view = self.view.read(cx);
                for (row, line) in view.viewport_lines.iter().enumerate() {
                    let y = bounds.top() + line_height * row as f32;
                    let runs = view.viewport_style_runs.get(row).map(|v| v.as_slice());
                    let mut run_idx: usize = 0;

                    let mut col = 1usize;
                    for ch in line.chars() {
                        let width = ch.width().unwrap_or(0);
                        if width == 0 {
                            continue;
                        }

                        if let Some((_, _)) = box_drawing_mask(ch) {
                            let fg = runs
                                .and_then(|runs| {
                                    while let Some(run) = runs.get(run_idx) {
                                        if (col as u16) <= run.end_col {
                                            break;
                                        }
                                        run_idx = run_idx.saturating_add(1);
                                    }
                                    runs.get(run_idx).and_then(|run| {
                                        (col as u16 >= run.start_col && (col as u16) <= run.end_col)
                                            .then_some(run)
                                    })
                                })
                                .map(|run| {
                                    let key = TextRunKey {
                                        fg: run.fg,
                                        flags: run.flags
                                            & (CELL_STYLE_FLAG_FAINT
                                                | CELL_STYLE_FLAG_BOLD
                                                | CELL_STYLE_FLAG_ITALIC
                                                | CELL_STYLE_FLAG_UNDERLINE
                                                | CELL_STYLE_FLAG_STRIKETHROUGH),
                                    };
                                    color_for_key(key)
                                })
                                .unwrap_or(default_fg);

                            let x = bounds.left() + px(cell_width * (col.saturating_sub(1)) as f32);
                            let cell_bounds =
                                Bounds::new(point(x, y), size(px(cell_width), line_height));
                            quads.extend(box_drawing_quads_for_char(
                                cell_bounds,
                                line_height,
                                cell_width,
                                fg,
                                ch,
                            ));
                        }

                        col = col.saturating_add(width);
                    }
                }

                quads
            })
            .unwrap_or_default();

        let cursor = {
            let view = self.view.read(cx);
            view.focus_handle
                .is_focused(window)
                .then(|| view.session.cursor_position())
                .flatten()
        }
        .and_then(|(col, row)| {
            let y = bounds.top() + line_height * (row.saturating_sub(1)) as f32;
            let row_index = row.saturating_sub(1) as usize;
            let line = shaped_lines.get(row_index)?;
            let byte_index = byte_index_for_column_in_line(line.text.as_str(), col);
            let x = bounds.left() + line.x_for_index(byte_index.min(line.text.len()));

            Some(fill(
                Bounds::new(point(x, y), size(px(2.0), line_height)),
                rgba(0xffffffaa),
            ))
        });

        TerminalPrepaintState {
            line_height,
            shaped_lines,
            background_quads,
            selection_quads,
            box_drawing_quads,
            marked_text,
            marked_text_background,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = { self.view.read(cx).focus_handle.clone() };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );

        window.paint_layer(bounds, |window| {
            let default_bg = { self.view.read(cx).session.default_background() };
            window.paint_quad(fill(bounds, hsla_from_rgb(default_bg)));

            for quad in prepaint.background_quads.drain(..) {
                window.paint_quad(quad);
            }

            for quad in prepaint.selection_quads.drain(..) {
                window.paint_quad(quad);
            }

            let origin = bounds.origin;
            for (row, line) in prepaint.shaped_lines.iter().enumerate() {
                let y = origin.y + prepaint.line_height * row as f32;
                let _ = line.paint(point(origin.x, y), prepaint.line_height, window, cx);
            }

            for quad in prepaint.box_drawing_quads.drain(..) {
                window.paint_quad(quad);
            }

            if let Some(bg) = prepaint.marked_text_background.take() {
                window.paint_quad(bg);
            }

            if let Some((line, origin)) = prepaint.marked_text.as_ref() {
                let _ = line.paint(*origin, prepaint.line_height, window, cx);
            }

            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.pending_output.is_empty() {
            let bytes = std::mem::take(&mut self.pending_output);
            if let Some(input) = self.input.as_ref() {
                let _ = self
                    .session
                    .feed_with_pty_responses(&bytes, |resp| input.send(resp));
            } else {
                let _ = self.session.feed(&bytes);
            }
            self.apply_side_effects(cx);
            let dirty = self.session.take_dirty_viewport_rows();
            if !dirty.is_empty() && !self.apply_dirty_viewport_rows(&dirty) {
                self.pending_refresh = true;
            }
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
            .font(self.font.clone())
            .whitespace_nowrap()
            .child(TerminalTextElement { view: cx.entity() })
    }
}

pub(crate) fn cell_metrics(window: &mut gpui::Window, font: &gpui::Font) -> Option<(f32, f32)> {
    let mut style = window.text_style();
    style.font_family = font.family.clone();
    style.font_features = crate::default_terminal_font_features();
    style.font_fallbacks = font.fallbacks.clone();

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

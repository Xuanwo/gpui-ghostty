use ghostty_vt::{Error, Rgb, Terminal};

use crate::TerminalConfig;

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
    dsr_state: DsrScanState,
    osc_query_state: OscQueryScanState,
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        let mut terminal = Terminal::new(config.cols, config.rows)?;
        terminal.set_default_colors(config.default_fg, config.default_bg);
        Ok(Self {
            config,
            terminal,
            bracketed_paste_enabled: false,
            mouse_x10_enabled: false,
            mouse_button_event_enabled: false,
            mouse_any_event_enabled: false,
            mouse_sgr_enabled: false,
            title: None,
            clipboard_write: None,
            parse_tail: Vec::new(),
            dsr_state: DsrScanState::default(),
            osc_query_state: OscQueryScanState::default(),
        })
    }

    pub fn cols(&self) -> u16 {
        self.config.cols
    }

    pub fn rows(&self) -> u16 {
        self.config.rows
    }

    pub fn default_foreground(&self) -> Rgb {
        self.config.default_fg
    }

    pub fn default_background(&self) -> Rgb {
        self.config.default_bg
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

    pub fn hyperlink_at(&self, col: u16, row: u16) -> Option<String> {
        self.terminal.hyperlink_at(col, row)
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

    pub fn feed_with_pty_responses(
        &mut self,
        bytes: &[u8],
        mut send: impl FnMut(&[u8]),
    ) -> Result<(), Error> {
        self.update_state_from_output(bytes);

        let mut seg_start = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            let dsr = self.dsr_state.advance(b);
            let osc = self.osc_query_state.advance(b);
            if dsr.is_none() && osc.is_none() {
                continue;
            }

            self.terminal.feed(&bytes[seg_start..=i])?;
            seg_start = i + 1;

            if let Some(query) = dsr {
                match query {
                    TerminalQuery::DeviceStatus => send(b"\x1b[0n"),
                    TerminalQuery::CursorPosition => {
                        let (col, row) = self.cursor_position().unwrap_or((1, 1));
                        let resp = format!("\x1b[{};{}R", row, col);
                        send(resp.as_bytes());
                    }
                }
            }

            if let Some(query) = osc {
                let rgb = match query {
                    OscQuery::ForegroundColor => {
                        let fg = self.config.default_fg;
                        (fg.r, fg.g, fg.b)
                    }
                    OscQuery::BackgroundColor => {
                        let bg = self.config.default_bg;
                        (bg.r, bg.g, bg.b)
                    }
                };
                let resp = osc_color_query_response(query, rgb);
                send(resp.as_bytes());
            }
        }

        if seg_start < bytes.len() {
            self.terminal.feed(&bytes[seg_start..])?;
        }

        Ok(())
    }

    pub fn dump_viewport(&self) -> Result<String, Error> {
        self.terminal.dump_viewport()
    }

    pub fn dump_viewport_row(&self, row: u16) -> Result<String, Error> {
        self.terminal.dump_viewport_row(row)
    }

    pub fn dump_viewport_row_cell_styles(
        &self,
        row: u16,
    ) -> Result<Vec<ghostty_vt::CellStyle>, Error> {
        self.terminal.dump_viewport_row_cell_styles(row)
    }

    pub fn dump_viewport_row_style_runs(
        &self,
        row: u16,
    ) -> Result<Vec<ghostty_vt::StyleRun>, Error> {
        self.terminal.dump_viewport_row_style_runs(row)
    }

    pub fn cursor_position(&self) -> Option<(u16, u16)> {
        self.terminal.cursor_position()
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
        self.config.cols = cols;
        self.config.rows = rows;
        self.terminal.resize(cols, rows)
    }

    pub(crate) fn take_dirty_viewport_rows(&mut self) -> Vec<u16> {
        self.terminal
            .take_dirty_viewport_rows(self.config.rows)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug)]
enum TerminalQuery {
    DeviceStatus,
    CursorPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OscQuery {
    ForegroundColor,
    BackgroundColor,
}

fn osc_color_query_response(query: OscQuery, (r, g, b): (u8, u8, u8)) -> String {
    let ps = match query {
        OscQuery::ForegroundColor => 10,
        OscQuery::BackgroundColor => 11,
    };

    let r16 = u16::from(r) * 0x0101;
    let g16 = u16::from(g) * 0x0101;
    let b16 = u16::from(b) * 0x0101;

    format!("\x1b]{};rgb:{:04x}/{:04x}/{:04x}\x1b\\", ps, r16, g16, b16)
}

#[derive(Clone, Copy, Debug, Default)]
enum DsrScanState {
    #[default]
    Idle,
    Esc,
    Csi,
    CsiQ,
    Csi5,
    CsiQ5,
    Csi6,
    CsiQ6,
}

impl DsrScanState {
    fn advance(&mut self, b: u8) -> Option<TerminalQuery> {
        use DsrScanState::*;

        let matched = match (*self, b) {
            (Csi5, b'n') | (CsiQ5, b'n') => Some(TerminalQuery::DeviceStatus),
            (Csi6, b'n') | (CsiQ6, b'n') => Some(TerminalQuery::CursorPosition),
            _ => None,
        };

        *self = match (*self, b) {
            (_, 0x1b) => Esc,
            (Esc, b'[') => Csi,
            (Csi, b'?') => CsiQ,
            (Csi, b'5') => Csi5,
            (CsiQ, b'5') => CsiQ5,
            (Csi, b'6') => Csi6,
            (CsiQ, b'6') => CsiQ6,
            (Csi5, b'n') => Idle,
            (CsiQ5, b'n') => Idle,
            (Csi6, b'n') => Idle,
            (CsiQ6, b'n') => Idle,
            _ => Idle,
        };

        matched
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum OscQueryScanState {
    #[default]
    Idle,
    Esc,
    Osc,
    Ps {
        value: u32,
    },
    AfterSemicolon {
        ps: u32,
    },
    Query {
        ps: u32,
    },
    StEscape {
        ps: u32,
    },
}

impl OscQueryScanState {
    fn advance(&mut self, b: u8) -> Option<OscQuery> {
        use OscQueryScanState::*;

        let matched = match (*self, b) {
            (Query { ps }, 0x07) => match ps {
                10 => Some(OscQuery::ForegroundColor),
                11 => Some(OscQuery::BackgroundColor),
                _ => None,
            },
            (StEscape { ps }, b'\\') => match ps {
                10 => Some(OscQuery::ForegroundColor),
                11 => Some(OscQuery::BackgroundColor),
                _ => None,
            },
            _ => None,
        };

        *self = match (*self, b) {
            (Query { ps }, 0x1b) => StEscape { ps },
            (_, 0x1b) => Esc,
            (Esc, b']') => Osc,
            (Esc, _) => Idle,
            (Osc, d) if d.is_ascii_digit() => Ps {
                value: (d - b'0') as u32,
            },
            (Ps { value }, d) if d.is_ascii_digit() => Ps {
                value: value.saturating_mul(10).saturating_add((d - b'0') as u32),
            },
            (Ps { value }, b';') => value_to_after_semicolon_state(value),
            (Osc, _) | (Ps { .. }, _) => Idle,
            (AfterSemicolon { ps }, b'?') => Query { ps },
            (AfterSemicolon { .. }, _) => Idle,
            (Query { .. }, 0x07) => Idle,
            (Query { .. }, _) => Idle,
            (StEscape { .. }, b'\\') => Idle,
            (StEscape { .. }, _) => Idle,
            _ => Idle,
        };

        matched
    }
}

fn value_to_after_semicolon_state(ps: u32) -> OscQueryScanState {
    match ps {
        10 | 11 => OscQueryScanState::AfterSemicolon { ps },
        _ => OscQueryScanState::Idle,
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

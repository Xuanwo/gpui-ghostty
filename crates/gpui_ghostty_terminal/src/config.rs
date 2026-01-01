use ghostty_vt::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct TerminalConfig {
    pub cols: u16,
    pub rows: u16,
    pub default_fg: Rgb,
    pub default_bg: Rgb,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            default_fg: Rgb {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF,
            },
            default_bg: Rgb {
                r: 0x00,
                g: 0x00,
                b: 0x00,
            },
        }
    }
}

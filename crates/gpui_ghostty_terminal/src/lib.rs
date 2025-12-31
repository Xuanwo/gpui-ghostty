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
    terminal: Terminal,
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        Ok(Self {
            terminal: Terminal::new(config.cols, config.rows)?,
        })
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.terminal.feed(bytes)
    }

    pub fn dump_viewport(&self) -> Result<String, Error> {
        self.terminal.dump_viewport()
    }
}

pub mod view {
    use super::TerminalSession;
    use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

    pub struct TerminalView {
        session: TerminalSession,
        viewport: String,
    }

    impl TerminalView {
        pub fn new(session: TerminalSession) -> Self {
            let viewport = session.dump_viewport().unwrap_or_default();
            Self { session, viewport }
        }
    }

    impl Render for TerminalView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.viewport = self.session.dump_viewport().unwrap_or_default();

            div()
                .size_full()
                .flex()
                .font_family("monospace")
                .whitespace_nowrap()
                .child(self.viewport.clone())
        }
    }
}

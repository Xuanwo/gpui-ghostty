use ghostty_vt::Terminal;

pub struct TerminalConfig;

pub struct TerminalSession {
    _terminal: Terminal,
}

impl TerminalSession {
    pub fn new(_config: TerminalConfig) -> Self {
        Self {
            _terminal: Terminal::new(),
        }
    }
}

#[cfg(feature = "gpui")]
pub mod view {
    use super::TerminalSession;
    use gpui::{div, prelude::*, Context, IntoElement, Render, Window};

    pub struct TerminalView {
        _session: TerminalSession,
    }

    impl TerminalView {
        pub fn new(session: TerminalSession) -> Self {
            Self { _session: session }
        }
    }

    impl Render for TerminalView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().flex().size_full()
        }
    }
}

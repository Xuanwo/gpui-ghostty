fn main() {
    use gpui::{App, AppContext, Application, WindowOptions};
    use gpui_ghostty_terminal::{TerminalConfig, TerminalSession};

    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| {
                let mut session = TerminalSession::new(TerminalConfig::default()).unwrap();
                session.feed(b"Hello from GPUI + Ghostty VT\r\n").unwrap();
                gpui_ghostty_terminal::view::TerminalView::new(session)
            })
        })
        .unwrap();
    });
}

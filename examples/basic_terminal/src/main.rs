fn main() {
    use gpui::{App, AppContext, Application, KeyBinding, WindowOptions};
    use gpui_ghostty_terminal::{TerminalConfig, TerminalSession, view::Copy, view::Paste};

    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-v", Paste, None),
        ]);

        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                focus_handle.focus(window);

                let mut session = TerminalSession::new(TerminalConfig::default()).unwrap();
                session.feed(b"Hello from GPUI + Ghostty VT\r\n").unwrap();
                gpui_ghostty_terminal::view::TerminalView::new(session, focus_handle)
            })
        })
        .unwrap();
    });
}

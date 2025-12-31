#[cfg(not(feature = "gpui"))]
fn main() {
    eprintln!("Enable the `gpui` feature to build this example.");
}

#[cfg(feature = "gpui")]
fn main() {
    use gpui::{App, AppContext, Application, WindowOptions};
    use gpui_ghostty_terminal::{TerminalConfig, TerminalSession};

    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| {
                let session = TerminalSession::new(TerminalConfig);
                gpui_ghostty_terminal::view::TerminalView::new(session)
            })
        })
        .unwrap();
    });
}

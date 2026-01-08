use ghostty_vt::Terminal;

#[test]
fn dirty_rows_include_entire_viewport_after_alt_screen_switch() {
    let mut terminal = Terminal::new(10, 5).unwrap();

    let _ = terminal.take_dirty_viewport_rows(5).unwrap();

    terminal.feed(b"\x1b[?1049h").unwrap();
    let _ = terminal.take_dirty_viewport_rows(5).unwrap();

    terminal.feed(b"x").unwrap();
    let _ = terminal.take_dirty_viewport_rows(5).unwrap();

    terminal.feed(b"\x1b[?1049l").unwrap();
    let dirty = terminal.take_dirty_viewport_rows(5).unwrap();

    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);

    let dirty_again = terminal.take_dirty_viewport_rows(5).unwrap();
    assert!(dirty_again.is_empty());
}

#[test]
fn dirty_rows_include_entire_viewport_after_reverse_colors_mode_change() {
    let mut terminal = Terminal::new(10, 5).unwrap();

    let _ = terminal.take_dirty_viewport_rows(5).unwrap();

    terminal.feed(b"\x1b[?5h").unwrap();
    let dirty = terminal.take_dirty_viewport_rows(5).unwrap();
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);

    let dirty_again = terminal.take_dirty_viewport_rows(5).unwrap();
    assert!(dirty_again.is_empty());
}

#[test]
fn dirty_rows_include_entire_viewport_after_palette_update() {
    let mut terminal = Terminal::new(10, 5).unwrap();

    let _ = terminal.take_dirty_viewport_rows(5).unwrap();

    terminal.feed(b"\x1b]4;1;#010203\x07").unwrap();
    let dirty = terminal.take_dirty_viewport_rows(5).unwrap();
    assert_eq!(dirty, vec![0, 1, 2, 3, 4]);

    let dirty_again = terminal.take_dirty_viewport_rows(5).unwrap();
    assert!(dirty_again.is_empty());
}

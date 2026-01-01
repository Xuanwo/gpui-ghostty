use ghostty_vt::{Rgb, Terminal};

#[test]
fn viewport_row_cell_styles_reflect_sgr_background() {
    let mut t = Terminal::new(4, 1).unwrap();
    t.feed(b"\x1b[41mX\x1b[0m").unwrap();

    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 4);

    assert_eq!(
        styles[0].bg,
        Rgb {
            r: 0xCC,
            g: 0x66,
            b: 0x66
        }
    );
    assert_eq!(
        styles[0].fg,
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF
        }
    );
}

#[test]
fn viewport_row_cell_styles_reflect_inverse() {
    let mut t = Terminal::new(2, 1).unwrap();
    t.feed(b"\x1b[7mX\x1b[0m").unwrap();

    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 2);

    assert_eq!(
        styles[0].bg,
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF
        }
    );
    assert_eq!(
        styles[0].fg,
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00
        }
    );
}

#[test]
fn viewport_row_cell_styles_reflect_faint_underline_and_strikethrough_flags() {
    let mut t = Terminal::new(4, 1).unwrap();
    t.feed(b"\x1b[2mX\x1b[0m").unwrap();
    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 4);
    assert_ne!(styles[0].flags & 0x10, 0);

    let mut t = Terminal::new(4, 1).unwrap();
    t.feed(b"\x1b[4mX\x1b[0m").unwrap();
    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 4);
    assert_ne!(styles[0].flags & 0x08, 0);

    let mut t = Terminal::new(4, 1).unwrap();
    t.feed(b"\x1b[9mX\x1b[0m").unwrap();
    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 4);
    assert_ne!(styles[0].flags & 0x40, 0);
}

#[test]
fn default_colors_can_be_overridden() {
    let mut t = Terminal::new(2, 1).unwrap();
    t.set_default_colors(
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00,
        },
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        },
    );
    t.feed(b"X").unwrap();

    let styles = t.dump_viewport_row_cell_styles(0).unwrap();
    assert_eq!(styles.len(), 2);
    assert_eq!(
        styles[0].fg,
        Rgb {
            r: 0x00,
            g: 0x00,
            b: 0x00
        }
    );
    assert_eq!(
        styles[0].bg,
        Rgb {
            r: 0xFF,
            g: 0xFF,
            b: 0xFF
        }
    );
}

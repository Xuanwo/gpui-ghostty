#[repr(C)]
pub struct ghostty_vt_bytes_t {
    pub ptr: *const u8,
    pub len: usize,
}

pub const PINNED_GHOSTTY_TAG: &str = "v1.2.3";
pub const PINNED_ZIG_VERSION: &str = "0.14.1";

unsafe extern "C" {
    pub fn ghostty_vt_terminal_new(cols: u16, rows: u16) -> *mut core::ffi::c_void;
    pub fn ghostty_vt_terminal_free(terminal: *mut core::ffi::c_void);

    pub fn ghostty_vt_terminal_feed(
        terminal: *mut core::ffi::c_void,
        bytes: *const u8,
        len: usize,
    ) -> core::ffi::c_int;

    pub fn ghostty_vt_terminal_resize(
        terminal: *mut core::ffi::c_void,
        cols: u16,
        rows: u16,
    ) -> core::ffi::c_int;

    pub fn ghostty_vt_terminal_scroll_viewport(
        terminal: *mut core::ffi::c_void,
        delta_lines: i32,
    ) -> core::ffi::c_int;

    pub fn ghostty_vt_terminal_scroll_viewport_top(
        terminal: *mut core::ffi::c_void,
    ) -> core::ffi::c_int;

    pub fn ghostty_vt_terminal_scroll_viewport_bottom(
        terminal: *mut core::ffi::c_void,
    ) -> core::ffi::c_int;

    pub fn ghostty_vt_terminal_cursor_position(
        terminal: *mut core::ffi::c_void,
        col_out: *mut u16,
        row_out: *mut u16,
    ) -> bool;

    pub fn ghostty_vt_terminal_dump_viewport(
        terminal: *mut core::ffi::c_void,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_terminal_dump_viewport_row(
        terminal: *mut core::ffi::c_void,
        row: u16,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_terminal_dump_viewport_row_cell_styles(
        terminal: *mut core::ffi::c_void,
        row: u16,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_terminal_take_dirty_viewport_rows(
        terminal: *mut core::ffi::c_void,
        rows: u16,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_terminal_hyperlink_at(
        terminal: *mut core::ffi::c_void,
        col: u16,
        row: u16,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_encode_key_named(
        name: *const u8,
        name_len: usize,
        modifiers: u16,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_bytes_free(bytes: ghostty_vt_bytes_t);
}

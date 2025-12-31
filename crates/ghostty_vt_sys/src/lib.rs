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

    pub fn ghostty_vt_terminal_dump_viewport(
        terminal: *mut core::ffi::c_void,
    ) -> ghostty_vt_bytes_t;

    pub fn ghostty_vt_bytes_free(bytes: ghostty_vt_bytes_t);
}

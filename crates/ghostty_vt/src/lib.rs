#[cfg(feature = "zig-build")]
mod zig {
    use std::ffi::c_void;
    use std::fmt;
    use std::ptr::NonNull;

    #[derive(Debug)]
    pub enum Error {
        CreateFailed,
        FeedFailed(i32),
        DumpFailed,
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Error::CreateFailed => write!(f, "terminal create failed"),
                Error::FeedFailed(code) => write!(f, "terminal feed failed: {code}"),
                Error::DumpFailed => write!(f, "terminal dump failed"),
            }
        }
    }

    impl std::error::Error for Error {}

    pub struct Terminal {
        ptr: NonNull<c_void>,
    }

    impl Terminal {
        pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
            let ptr = unsafe { ghostty_vt_sys::ghostty_vt_terminal_new(cols, rows) };
            let ptr = NonNull::new(ptr).ok_or(Error::CreateFailed)?;
            Ok(Self { ptr })
        }

        pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
            let rc = unsafe {
                ghostty_vt_sys::ghostty_vt_terminal_feed(self.ptr.as_ptr(), bytes.as_ptr(), bytes.len())
            };
            if rc == 0 {
                Ok(())
            } else {
                Err(Error::FeedFailed(rc))
            }
        }

        pub fn dump_viewport(&self) -> Result<String, Error> {
            let bytes = unsafe { ghostty_vt_sys::ghostty_vt_terminal_dump_viewport(self.ptr.as_ptr()) };
            if bytes.ptr.is_null() {
                return Err(Error::DumpFailed);
            }

            let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
            let s = String::from_utf8_lossy(slice).into_owned();
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            Ok(s)
        }
    }

    impl Drop for Terminal {
        fn drop(&mut self) {
            unsafe { ghostty_vt_sys::ghostty_vt_terminal_free(self.ptr.as_ptr()) }
        }
    }

    pub fn terminal_new(cols: u16, rows: u16) -> Result<Terminal, Error> {
        Terminal::new(cols, rows)
    }
}

#[cfg(feature = "zig-build")]
pub use zig::{terminal_new, Error, Terminal};

#[cfg(not(feature = "zig-build"))]
#[derive(Debug)]
pub enum Error {
    ZigBuildDisabled,
}

#[cfg(not(feature = "zig-build"))]
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ZigBuildDisabled => write!(f, "zig-build feature is disabled"),
        }
    }
}

#[cfg(not(feature = "zig-build"))]
impl std::error::Error for Error {}

#[cfg(not(feature = "zig-build"))]
pub struct Terminal;

#[cfg(not(feature = "zig-build"))]
impl Terminal {
    pub fn new(_cols: u16, _rows: u16) -> Result<Self, Error> {
        Err(Error::ZigBuildDisabled)
    }
}

#[cfg(not(feature = "zig-build"))]
pub fn terminal_new(cols: u16, rows: u16) -> Result<Terminal, Error> {
    Terminal::new(cols, rows)
}

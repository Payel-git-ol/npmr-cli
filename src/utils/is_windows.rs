//! Port of `lib/utils/is-windows.js`.

pub fn is_windows() -> bool {
    cfg!(windows)
}

/// True when the current shell on Windows is cmd.exe or PowerShell, the
/// cases where `npm run` needs platform specific shims.
pub fn is_windows_shell() -> bool {
    is_windows()
}

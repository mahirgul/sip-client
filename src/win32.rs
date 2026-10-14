//! Windows Win32 integration for native Windows dialog and console handling.

#[cfg(windows)]
pub fn show_win32_dialog(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(std::ptr::null_mut(), msg_wide.as_ptr(), title_wide.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }
}

#[cfg(not(windows))]
pub fn show_win32_dialog(_title: &str, _message: &str) {
    println!("[{_title}] {_message}");
}

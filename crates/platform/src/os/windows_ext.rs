//! Windows extras: system settings, window effects, rich clipboard, URL scheme
//! registration. Type-checked from Linux, built by CI, never run by the author.

use crate::settings::{self, SystemSettings};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn reg_dword(key: &str, value: &str) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: out-pointers reference live locals of the stated size; the key
    // and value names are NUL-terminated UTF-16 that outlive the call.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(key).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (rc == 0).then_some(data)
}

/// Dark mode and accent from the registry, reduced motion and high contrast
/// from `SystemParametersInfoW`.
pub fn read_settings() -> SystemSettings {
    use windows_sys::Win32::UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
        },
    };
    let mut animations: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through pvParam.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut animations as *mut i32).cast(), 0)
    };
    // SAFETY: HIGHCONTRASTW is plain data; all-zero is a valid value.
    let mut hc: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
    hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    // SAFETY: SPI_GETHIGHCONTRAST fills the HIGHCONTRASTW whose cbSize we set.
    let hc_ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, (&mut hc as *mut HIGHCONTRASTW).cast(), 0)
    };
    SystemSettings {
        dark: super::windows::is_dark().unwrap_or(false),
        reduced_motion: ok != 0 && animations == 0,
        high_contrast: hc_ok != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0,
        accent: reg_dword(r"Software\Microsoft\Windows\DWM", "AccentColor")
            .map(settings::parse_windows_accent),
    }
}

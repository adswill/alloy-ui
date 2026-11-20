//! Windows backend using PowerShell and `reg.exe`, both present on every
//! supported Windows. Compiled by CI on Windows; not run by the author.

use super::{capture, feed};
use crate::{theme, PlatformError};
use std::path::PathBuf;

fn ps(script: &str) -> Option<String> {
    capture(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )
}

pub fn clipboard_read() -> Option<String> {
    ps("Get-Clipboard -Raw").map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
}

pub fn clipboard_write(text: &str) -> bool {
    feed("clip", &[], text)
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; \
        $d = New-Object System.Windows.Forms.OpenFileDialog; \
        if ($d.ShowDialog() -eq 'OK') { $d.FileName }";
    Ok(ps(script)
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty()))
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    let q = |s: &str| s.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         [void][System.Windows.Forms.MessageBox]::Show('{}','{}')",
        q(body),
        q(title)
    );
    ps(&script);
    Ok(())
}

pub fn is_dark() -> Option<bool> {
    let out = capture(
        "reg",
        &[
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ],
    );
    theme::parse_windows(out.as_deref())
}

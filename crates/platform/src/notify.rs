//! Desktop notifications through each OS's own service: the freedesktop
//! `org.freedesktop.Notifications` D-Bus interface on Linux, Notification
//! Center on macOS and toast notifications on Windows.

use crate::PlatformError;

/// A notification to show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Notification {
    /// Bold first line.
    pub title: String,
    /// Text below the title.
    pub body: String,
    /// How long the desktop keeps it, in milliseconds. `None` lets the OS decide.
    pub timeout_ms: Option<u32>,
}

/// Show `n`. Returns once the OS has accepted it, not when the user has seen it.
/// Errors when the OS has no notification service (a bare window manager).
pub fn send(n: &Notification) -> Result<(), PlatformError> {
    crate::os::notify(n)
}

/// AppleScript that posts `n` to Notification Center.
#[allow(dead_code)] // only called by the macOS backend
pub(crate) fn applescript(n: &Notification) -> String {
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "display notification \"{}\" with title \"{}\"",
        q(&n.body),
        q(&n.title)
    )
}

/// PowerShell that shows a toast. It borrows PowerShell's own app id, which is
/// what lets an unpackaged program show toasts without an installer.
#[allow(dead_code)] // only called by the Windows backend
pub(crate) fn powershell_toast(n: &Notification) -> String {
    let x = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    };
    // The XML sits in a single-quoted PowerShell string, so `'` is written as `&apos;`.
    format!(
        "$ErrorActionPreference='Stop';\
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null;\
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null;\
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument;\
$xml.LoadXml('<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>');\
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml);\
$id = '{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe';\
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($id).Show($toast)",
        x(&n.title),
        x(&n.body)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n() -> Notification {
        Notification {
            title: "Done \"now\"".into(),
            body: "a <b> & c".into(),
            timeout_ms: None,
        }
    }

    #[test]
    fn applescript_quotes() {
        assert_eq!(
            applescript(&n()),
            "display notification \"a <b> & c\" with title \"Done \\\"now\\\"\""
        );
    }

    #[test]
    fn toast_escapes_xml() {
        let s = powershell_toast(&n());
        assert!(s.contains("Done &quot;now&quot;"));
        assert!(s.contains("a &lt;b&gt; &amp; c"));
        assert!(s.contains("CreateToastNotifier"));
    }
}

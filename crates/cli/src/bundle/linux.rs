//! Pieces shared by the `.deb` and AppImage bundlers.

use super::Ctx;
use crate::error::{Error, Result};

/// Render the freedesktop `.desktop` file.
pub fn desktop_entry(ctx: &Ctx<'_>) -> String {
    let p = ctx.project;
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    // `%U` hands the launcher's URLs and files to the app as arguments.
    let (exec, mime) = if p.url_schemes.is_empty() {
        (p.bin_name.clone(), String::new())
    } else {
        let types: String = p
            .url_schemes
            .iter()
            .map(|s| format!("x-scheme-handler/{s};"))
            .collect();
        (format!("{} %U", p.bin_name), format!("MimeType={types}\n"))
    };
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nIcon={}\nTerminal=false\nCategories={};\nStartupWMClass={}\n{mime}",
        p.product_name,
        one_line(&p.description),
        exec,
        p.bin_name,
        p.category,
        p.bin_name,
    )
}

/// Install-time location of the app's data directory: `usr/share/<bin>`.
pub fn share_dir(ctx: &Ctx<'_>) -> String {
    format!("usr/share/{}", ctx.project.bin_name)
}

/// Reject targets that are not Linux.
pub fn require_linux(ctx: &Ctx<'_>, format: &'static str) -> Result<()> {
    if ctx.triple.contains("linux") {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "`{format}` bundles Linux binaries but the target is `{}`; pass --target <linux triple>",
            ctx.triple
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::fixture;

    #[test]
    fn desktop_file_contents() {
        let f = fixture();
        let d = desktop_entry(&f.ctx());
        assert!(d.starts_with("[Desktop Entry]\n"));
        assert!(d.contains("\nName=Demo App\n"));
        assert!(d.contains("\nExec=demo\n"));
        assert!(d.contains("\nCategories=Graphics;\n"));
        assert!(!d.contains("\n\n"), "no blank lines in a desktop file");
        assert!(!d.contains("MimeType"), "no scheme handler unless declared");
    }

    #[test]
    fn desktop_file_declares_url_schemes() {
        let mut f = fixture();
        f.project.url_schemes = vec!["demo".into(), "demo-dev".into()];
        let d = desktop_entry(&f.ctx());
        assert!(d.contains("\nExec=demo %U\n"));
        assert!(d.contains("\nMimeType=x-scheme-handler/demo;x-scheme-handler/demo-dev;\n"));
        assert!(!d.contains("\n\n"));
    }
}

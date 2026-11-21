//! The system clipboard beyond text: HTML, images and files, one call each.
//!
//! ```ignore
//! use alloy_ui::clipboard;
//!
//! clipboard::copy_html("<b>Q3</b> revenue", "Q3 revenue");   // rich paste into mail, docs
//! clipboard::copy_image(w, h, &rgba);                         // paste into an image editor
//! clipboard::copy_files(["report.pdf"]);                      // paste into a file manager
//! let clip = clipboard::paste();                              // text, html, image, files
//! ```
//!
//! Every call returns quickly; `paste` waits for the owning app to answer (at most about a
//! second), so call it from an event handler, not every frame.

pub use alloy_ui_platform::clipboard::{html_to_text, ClipContent, ClipImage};
use alloy_ui_platform::default_clipboard;
use std::path::{Path, PathBuf};

fn write(c: &ClipContent) -> bool {
    default_clipboard().write_rich(c).is_ok()
}

/// Put plain text on the clipboard.
pub fn copy(text: impl AsRef<str>) -> bool {
    write(&ClipContent::text(text.as_ref()))
}

/// Put an HTML fragment on the clipboard, with `plain` for apps that only paste text. Pass
/// `""` as `plain` to derive it by stripping the tags.
pub fn copy_html(html: &str, plain: &str) -> bool {
    let plain = if plain.is_empty() {
        html_to_text(html)
    } else {
        plain.to_owned()
    };
    write(&ClipContent::html(html, plain))
}

/// Put an image on the clipboard: `rgba` is `width * height * 4` bytes of straight RGBA, top row
/// first. False if the length does not match.
pub fn copy_image(width: u32, height: u32, rgba: &[u8]) -> bool {
    ClipContent::image(width, height, rgba.to_vec()).is_some_and(|c| write(&c))
}

/// Put a PNG file's picture on the clipboard.
pub fn copy_image_file(path: impl AsRef<Path>) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|b| alloy_ui_platform::clipboard::decode_png(&b))
        .is_some_and(|i| write(&ClipContent {
            image: Some(i),
            ..ClipContent::default()
        }))
}

/// Put files on the clipboard the way a file manager's "Copy" does, so a paste there copies them.
pub fn copy_files<P: Into<PathBuf>>(files: impl IntoIterator<Item = P>) -> bool {
    write(&ClipContent::files(files))
}

/// Everything on the clipboard: text, HTML, an image, files. Fields are empty when absent.
pub fn paste() -> ClipContent {
    default_clipboard().read_rich().unwrap_or_default()
}

/// Just the text on the clipboard.
pub fn paste_text() -> Option<String> {
    default_clipboard().read_text().ok().flatten()
}

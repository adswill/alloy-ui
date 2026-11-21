//! Rich clipboard content (text, HTML, images, files) and the pure encoders
//! and parsers the OS backends share. Everything here is tested on every OS.

use std::path::{Path, PathBuf};

/// A decoded image: straight (non-premultiplied) RGBA, 8 bits per channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, row-major, top row first.
    pub rgba: Vec<u8>,
}

/// Everything that can sit on the clipboard at once. Every field is optional:
/// a copy from a browser usually carries `text` and `html`, a screenshot
/// carries only an `image`, a file manager only `files`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClipContent {
    /// Plain text.
    pub text: Option<String>,
    /// An HTML fragment.
    pub html: Option<String>,
    /// A bitmap.
    pub image: Option<ClipImage>,
    /// Files (paths), as a file manager copies them.
    pub files: Vec<PathBuf>,
}

impl ClipContent {
    /// Nothing on the clipboard.
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.html.is_none() && self.image.is_none() && self.files.is_empty()
    }
    /// Plain text only.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            ..Self::default()
        }
    }
    /// An HTML fragment, with `plain` as the fallback for apps that only paste text.
    pub fn html(html: impl Into<String>, plain: impl Into<String>) -> Self {
        Self {
            html: Some(html.into()),
            text: Some(plain.into()),
            ..Self::default()
        }
    }
    /// A bitmap from straight RGBA bytes. `None` if the length does not match the size.
    pub fn image(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        (rgba.len() == width as usize * height as usize * 4 && width > 0 && height > 0).then(|| {
            Self {
                image: Some(ClipImage {
                    width,
                    height,
                    rgba,
                }),
                ..Self::default()
            }
        })
    }
    /// Files, as a file manager's "Copy" puts them.
    pub fn files(files: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            files: files.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }
}

/// Strip tags from an HTML fragment for a plain-text fallback. Not a parser:
/// good enough for the text that goes next to the HTML on the clipboard.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                in_tag = true;
                let rest: String = chars.clone().take(3).collect();
                if rest.starts_with("br") || rest.starts_with("p>") || rest.starts_with("/p") {
                    out.push('\n');
                }
            }
            '>' if in_tag => in_tag = false,
            '&' if !in_tag => {
                let mut ent = String::new();
                while let Some(&n) = chars.peek() {
                    if n == ';' || ent.len() > 6 {
                        break;
                    }
                    ent.push(n);
                    chars.next();
                }
                if chars.peek() == Some(&';') {
                    chars.next();
                    match ent.as_str() {
                        "amp" => out.push('&'),
                        "lt" => out.push('<'),
                        "gt" => out.push('>'),
                        "quot" => out.push('"'),
                        "nbsp" => out.push(' '),
                        "#39" | "apos" => out.push('\''),
                        other => {
                            out.push('&');
                            out.push_str(other);
                            out.push(';');
                        }
                    }
                } else {
                    out.push('&');
                    out.push_str(&ent);
                }
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Encode as PNG (what every OS clipboard accepts for images).
pub fn encode_png(img: &ClipImage) -> Option<Vec<u8>> {
    if img.rgba.len() != img.width as usize * img.height as usize * 4 {
        return None;
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().ok()?;
    w.write_image_data(&img.rgba).ok()?;
    w.finish().ok()?;
    Some(out)
}

/// Decode a PNG of any colour type to RGBA8.
pub fn decode_png(bytes: &[u8]) -> Option<ClipImage> {
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None, // EXPAND already resolves these
    };
    Some(ClipImage {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// Percent-encode a path for a `file://` URI (keeps `/` and unreserved characters).
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Undo [`percent_encode`]. Invalid escapes are kept as they are.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `text/uri-list` for these files (CRLF separated, as RFC 2483 asks).
pub fn uri_list(files: &[PathBuf]) -> String {
    files
        .iter()
        .map(|p| format!("file://{}\r\n", percent_encode(&p.to_string_lossy())))
        .collect()
}

/// Parse a `text/uri-list` (also the `x-special/gnome-copied-files` body):
/// `file://` URIs become paths, comments, blank lines and the `copy`/`cut`
/// header line are skipped.
pub fn parse_uri_list(s: &str) -> Vec<PathBuf> {
    s.lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("file://"))
        .map(|rest| {
            // `file://host/path`: drop an empty or `localhost` host.
            let path = rest.strip_prefix("localhost").unwrap_or(rest);
            PathBuf::from(percent_decode(path))
        })
        .collect()
}

/// Wrap an HTML fragment in the Windows "HTML Format" clipboard envelope
/// (header with byte offsets, then the document with fragment markers).
pub fn cf_html(fragment: &str) -> String {
    const PRE: &str = "<html><body>\r\n<!--StartFragment-->";
    const POST: &str = "<!--EndFragment-->\r\n</body></html>";
    // Offsets are fixed-width so the header length is known up front.
    let header_len = "Version:0.9\r\nStartHTML:00000000\r\nEndHTML:00000000\r\nStartFragment:00000000\r\nEndFragment:00000000\r\n".len();
    let start_html = header_len;
    let start_frag = start_html + PRE.len();
    let end_frag = start_frag + fragment.len();
    let end_html = end_frag + POST.len();
    format!(
        "Version:0.9\r\nStartHTML:{start_html:08}\r\nEndHTML:{end_html:08}\r\nStartFragment:{start_frag:08}\r\nEndFragment:{end_frag:08}\r\n{PRE}{fragment}{POST}"
    )
}

/// Extract the fragment from a Windows "HTML Format" payload.
pub fn parse_cf_html(s: &str) -> Option<String> {
    let field = |name: &str| -> Option<usize> {
        let line = s.lines().find(|l| l.starts_with(name))?;
        line[name.len()..].trim().parse().ok()
    };
    let (a, b) = (field("StartFragment:")?, field("EndFragment:")?);
    s.as_bytes()
        .get(a..b)
        .map(|f| String::from_utf8_lossy(f).into_owned())
}

/// The clipboard formats (MIME-style names) to offer for `c`, in preference
/// order. Used by the Linux backends; both X11 targets and Wayland MIME types
/// are strings, so one table serves both.
pub fn to_formats(c: &ClipContent) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    if let Some(img) = &c.image {
        if let Some(png) = encode_png(img) {
            out.push(("image/png".into(), png));
        }
    }
    if let Some(h) = &c.html {
        out.push(("text/html".into(), h.clone().into_bytes()));
    }
    if !c.files.is_empty() {
        let list = uri_list(&c.files);
        out.push(("text/uri-list".into(), list.clone().into_bytes()));
        let gnome = format!(
            "copy\n{}",
            list.replace("\r\n", "\n").trim_end_matches('\n')
        );
        out.push(("x-special/gnome-copied-files".into(), gnome.into_bytes()));
    }
    // Files: file managers also paste the names as text, so offer them.
    let text = c.text.clone().or_else(|| {
        (!c.files.is_empty()).then(|| {
            c.files
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n")
        })
    });
    if let Some(t) = text {
        for m in TEXT_FORMATS {
            out.push(((*m).into(), t.clone().into_bytes()));
        }
    }
    out
}

/// Text formats, best first.
pub const TEXT_FORMATS: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "TEXT",
    "STRING",
];

/// Assemble a [`ClipContent`] from what a selection `offered`, fetching each
/// wanted format through `fetch` (which returns its bytes).
pub fn from_formats(
    offered: &[String],
    fetch: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
) -> ClipContent {
    let has = |m: &str| offered.iter().any(|o| o == m);
    let mut c = ClipContent::default();
    if has("image/png") {
        c.image = fetch("image/png").and_then(|b| decode_png(&b));
    }
    if has("text/html") {
        c.html = fetch("text/html").map(|b| String::from_utf8_lossy(&b).into_owned());
    }
    for m in ["text/uri-list", "x-special/gnome-copied-files"] {
        if c.files.is_empty() && has(m) {
            c.files = fetch(m)
                .map(|b| parse_uri_list(&String::from_utf8_lossy(&b)))
                .unwrap_or_default();
        }
    }
    if let Some(m) = TEXT_FORMATS.iter().find(|m| has(m)) {
        c.text = fetch(m).map(|b| String::from_utf8_lossy(&b).into_owned());
    }
    c
}

/// Is `p` an absolute path to something that exists? Used to tell files from
/// URLs among launch arguments.
pub(crate) fn is_existing_path(p: &str) -> bool {
    Path::new(p).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ClipImage {
        ClipImage {
            width: 3,
            height: 2,
            rgba: (0..24).map(|i| i as u8 * 9).collect(),
        }
    }

    #[test]
    fn png_roundtrip() {
        let img = sample();
        let bytes = encode_png(&img).unwrap();
        assert_eq!(&bytes[..4], b"\x89PNG");
        assert_eq!(decode_png(&bytes), Some(img));
        assert_eq!(decode_png(b"not a png"), None);
        assert!(encode_png(&ClipImage {
            width: 2,
            height: 2,
            rgba: vec![0; 3]
        })
        .is_none());
    }

    #[test]
    fn uri_lists() {
        let files = vec![
            PathBuf::from("/home/a b/ünï.txt"),
            PathBuf::from("/tmp/x#1.png"),
        ];
        let l = uri_list(&files);
        assert!(l.contains("file:///home/a%20b/%C3%BCn%C3%AF.txt\r\n"));
        assert_eq!(parse_uri_list(&l), files);
        assert_eq!(
            parse_uri_list("# c\ncopy\nfile://localhost/x/y\n\nfile:///z"),
            vec![PathBuf::from("/x/y"), PathBuf::from("/z")]
        );
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn windows_html_format() {
        let s = cf_html("<b>hi</b> ü");
        assert_eq!(parse_cf_html(&s).as_deref(), Some("<b>hi</b> ü"));
        // StartHTML points at the start of the document.
        let n: usize = s
            .lines()
            .find(|l| l.starts_with("StartHTML:"))
            .unwrap()[10..]
            .parse()
            .unwrap();
        assert!(s[n..].starts_with("<html>"));
        assert_eq!(parse_cf_html("garbage"), None);
    }

    #[test]
    fn plain_text_from_html() {
        assert_eq!(
            html_to_text("<p>Fish &amp; <b>chips</b></p><p>2 &lt; 3</p>"),
            "Fish & chips\n\n2 < 3"
        );
        assert_eq!(html_to_text("a<br>b"), "a\nb");
    }

    #[test]
    fn formats_roundtrip() {
        let mut c = ClipContent::html("<i>x</i>", "x");
        c.image = Some(sample());
        c.files = vec![PathBuf::from("/tmp/a.txt")];
        let formats = to_formats(&c);
        let offered: Vec<String> = formats.iter().map(|(m, _)| m.clone()).collect();
        assert!(offered.contains(&"image/png".to_string()));
        assert!(offered.contains(&"x-special/gnome-copied-files".to_string()));
        let back = from_formats(&offered, &mut |m| {
            formats.iter().find(|(n, _)| n == m).map(|(_, d)| d.clone())
        });
        assert_eq!(back, c);
    }

    #[test]
    fn constructors() {
        assert!(ClipContent::default().is_empty());
        assert!(ClipContent::image(2, 2, vec![0; 15]).is_none());
        assert!(ClipContent::image(2, 2, vec![0; 16]).is_some());
        assert_eq!(ClipContent::files(["/a"]).files.len(), 1);
    }
}

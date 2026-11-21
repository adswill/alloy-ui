//! MSIX package, written entirely in Rust (no `makeappx` needed), so it can be
//! produced and structurally verified on any host.
//!
//! An MSIX is a zip holding `AppxManifest.xml`, `AppxBlockMap.xml` (SHA-256 of
//! every 64 KiB block of every file), `[Content_Types].xml` and the app files.
//! We store entries uncompressed, which keeps the block map trivial (no
//! per-block compressed sizes). The result is **unsigned** unless Windows
//! signing is configured (see `sign`); Windows refuses to install an unsigned
//! MSIX, and the manifest `Publisher` must equal the certificate subject.

use super::sha256::sha256;
use super::sign::win_sign_file;
use super::windows::{exe_with_icon, require_windows, version_quad};
use super::{write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use std::path::PathBuf;

const BLOCK: usize = 64 * 1024;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Standard base64 with padding.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *t = c;
    }
    !data.iter().fold(!0u32, |c, &b| {
        table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8)
    })
}

/// Stored (uncompressed) zip with a fixed timestamp, so builds are reproducible.
#[derive(Default)]
pub struct Zip {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Zip {
    /// Append a file. Returns the size of its local header (`LfhSize` in the block map).
    pub fn add(&mut self, name: &str, data: &[u8]) -> u32 {
        let offset = self.out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        let mut common = Vec::new();
        common.extend(20u16.to_le_bytes()); // version needed
        common.extend(0u16.to_le_bytes()); // flags
        common.extend(0u16.to_le_bytes()); // method: stored
        common.extend(0u16.to_le_bytes()); // time 00:00
        common.extend(0x0021u16.to_le_bytes()); // date 1980-01-01
        common.extend(crc.to_le_bytes());
        common.extend((data.len() as u32).to_le_bytes());
        common.extend((data.len() as u32).to_le_bytes());
        common.extend((n.len() as u16).to_le_bytes());
        common.extend(0u16.to_le_bytes()); // extra length
        self.out.extend(0x0403_4b50u32.to_le_bytes());
        self.out.extend(&common);
        self.out.extend(n);
        self.out.extend(data);
        let lfh = 30 + n.len() as u32;
        self.central.extend(0x0201_4b50u32.to_le_bytes());
        self.central.extend(20u16.to_le_bytes()); // version made by
        self.central.extend(&common);
        self.central.extend(0u16.to_le_bytes()); // comment length
        self.central.extend(0u16.to_le_bytes()); // disk
        self.central.extend(0u16.to_le_bytes()); // internal attrs
        self.central.extend(0u32.to_le_bytes()); // external attrs
        self.central.extend(offset.to_le_bytes());
        self.central.extend(n);
        self.count += 1;
        lfh
    }

    /// Finish the archive.
    pub fn finish(mut self) -> Vec<u8> {
        let start = self.out.len() as u32;
        let cd = std::mem::take(&mut self.central);
        self.out.extend(&cd);
        self.out.extend(0x0605_4b50u32.to_le_bytes());
        self.out.extend(0u32.to_le_bytes());
        self.out.extend(self.count.to_le_bytes());
        self.out.extend(self.count.to_le_bytes());
        self.out.extend((cd.len() as u32).to_le_bytes());
        self.out.extend(start.to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        self.out
    }
}

/// `AppxBlockMap.xml` for `files` as `(zip name, bytes, lfh size)`.
pub fn block_map(files: &[(String, &[u8], u32)]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<BlockMap xmlns=\"http://schemas.microsoft.com/appx/2010/blockmap\" HashMethod=\"http://www.w3.org/2001/04/xmlenc#sha256\">\n",
    );
    for (name, data, lfh) in files {
        s.push_str(&format!(
            "  <File Name=\"{}\" Size=\"{}\" LfhSize=\"{lfh}\">\n",
            esc(&name.replace('/', "\\")),
            data.len()
        ));
        for block in data.chunks(BLOCK) {
            s.push_str(&format!(
                "    <Block Hash=\"{}\" />\n",
                base64(&sha256(block))
            ));
        }
        s.push_str("  </File>\n");
    }
    s.push_str("</BlockMap>\n");
    s
}

fn content_types() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n  <Default Extension=\"png\" ContentType=\"image/png\" />\n  <Default Extension=\"exe\" ContentType=\"application/x-msdownload\" />\n  <Default Extension=\"toml\" ContentType=\"application/octet-stream\" />\n  <Override PartName=\"/AppxManifest.xml\" ContentType=\"application/vnd.ms-appx.manifest+xml\" />\n  <Override PartName=\"/AppxBlockMap.xml\" ContentType=\"application/vnd.ms-appx.blockmap+xml\" />\n</Types>\n"
}

/// `Identity/@Publisher`: the configured subject, else `CN=<publisher>`.
pub fn publisher(ctx: &Ctx<'_>) -> String {
    let p = ctx.project;
    p.msix_publisher
        .clone()
        .unwrap_or_else(|| format!("CN={}", p.publisher.as_deref().unwrap_or(&p.product_name)))
}

/// Render `AppxManifest.xml`.
pub fn manifest(ctx: &Ctx<'_>) -> Result<String> {
    let p = ctx.project;
    let ver = version_quad(&p.version)
        .map(|q| {
            q.rsplit_once('.')
                .map_or("0.0.0".to_string(), |x| x.0.to_string())
        })
        .ok_or_else(|| {
            Error::Config(format!(
                "version `{}` must start with numeric major.minor.patch for an MSIX",
                p.version
            ))
        })?;
    // The Store reserves the fourth part, so it is always 0.
    let ver = format!("{ver}.0");
    let arch = match ctx.arch.as_str() {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "i686" | "i586" => "x86",
        other => return Err(Error::Config(format!("no MSIX architecture for `{other}`"))),
    };
    if !(3..=50).contains(&p.identifier.len()) {
        return Err(Error::Config(
            "an MSIX identity name needs 3 to 50 characters; shorten `identifier`".into(),
        ));
    }
    let name = esc(&p.product_name);
    let desc = esc(&p
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "));
    let bin = esc(&format!("{}.exe", p.bin_name));
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<Package xmlns=\"http://schemas.microsoft.com/appx/manifest/foundation/windows10\" xmlns:uap=\"http://schemas.microsoft.com/appx/manifest/uap/windows10\" xmlns:rescap=\"http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities\" IgnorableNamespaces=\"uap rescap\">\n\
  <Identity Name=\"{id}\" Publisher=\"{publisher}\" Version=\"{ver}\" ProcessorArchitecture=\"{arch}\" />\n\
  <Properties>\n    <DisplayName>{name}</DisplayName>\n    <PublisherDisplayName>{pubname}</PublisherDisplayName>\n    <Logo>Assets\\StoreLogo.png</Logo>\n  </Properties>\n\
  <Dependencies>\n    <TargetDeviceFamily Name=\"Windows.Desktop\" MinVersion=\"10.0.17763.0\" MaxVersionTested=\"10.0.22631.0\" />\n  </Dependencies>\n\
  <Resources>\n    <Resource Language=\"en-us\" />\n  </Resources>\n\
  <Applications>\n    <Application Id=\"App\" Executable=\"{bin}\" EntryPoint=\"Windows.FullTrustApplication\">\n      <uap:VisualElements DisplayName=\"{name}\" Description=\"{desc}\" BackgroundColor=\"transparent\" Square150x150Logo=\"Assets\\Square150x150Logo.png\" Square44x44Logo=\"Assets\\Square44x44Logo.png\" />\n    </Application>\n  </Applications>\n\
  <Capabilities>\n    <rescap:Capability Name=\"runFullTrust\" />\n  </Capabilities>\n\
</Package>\n",
        id = esc(&p.identifier),
        publisher = esc(&publisher(ctx)),
        pubname = esc(p.publisher.as_deref().unwrap_or(&p.product_name)),
    ))
}

/// The package bytes (unsigned).
pub fn build(ctx: &Ctx<'_>) -> Result<Vec<u8>> {
    let icon = ctx.icons.largest_png().ok_or_else(|| {
        Error::Config(
            "msix needs at least one PNG icon (set `icons` in [package.metadata.alloy])".into(),
        )
    })?;
    let exe = exe_with_icon(ctx)?;
    let manifest = manifest(ctx)?;
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("AppxManifest.xml".into(), manifest.into_bytes()),
        (format!("{}.exe", ctx.project.bin_name), exe),
    ];
    for logo in ["StoreLogo", "Square44x44Logo", "Square150x150Logo"] {
        files.push((format!("Assets/{logo}.png"), icon.data.clone()));
    }
    if let Some(c) = &ctx.capabilities {
        files.push((CAPABILITIES_FILE.into(), c.clone().into_bytes()));
    }
    let mut zip = Zip::default();
    let ct = content_types().as_bytes().to_vec();
    zip.add("[Content_Types].xml", &ct);
    let mut mapped = Vec::new();
    for (name, data) in &files {
        let lfh = zip.add(name, data);
        mapped.push((name.clone(), data.as_slice(), lfh));
    }
    zip.add("AppxBlockMap.xml", block_map(&mapped).as_bytes());
    Ok(zip.finish())
}

/// Build `<stem>.msix`, signing it when Windows signing is configured.
pub fn msix(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_windows(ctx, "msix")?;
    let out = ctx.out_dir.join(format!("{}.msix", ctx.stem()));
    write_file(&out, &build(ctx)?)?;
    if super::sign::WinSign::from_env(&super::sign::process_env, &super::which)?.is_some() {
        win_sign_file(&out)?;
    } else {
        eprintln!(
            "note: {} is unsigned; Windows will not install it until it is signed with a certificate whose subject is `{}` (see ALLOY_WIN_CERT)",
            out.display(),
            publisher(ctx)
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    #[test]
    fn base64_and_crc_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn manifest_contents() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        let m = manifest(&f.ctx()).unwrap();
        for needle in [
            "Name=\"dev.example.demo\"",
            "Publisher=\"CN=Example\"",
            "Version=\"1.2.3.0\"",
            "ProcessorArchitecture=\"x64\"",
            "<DisplayName>Demo App</DisplayName>",
            "Executable=\"demo.exe\"",
            "EntryPoint=\"Windows.FullTrustApplication\"",
            "runFullTrust",
        ] {
            assert!(m.contains(needle), "missing {needle}\n{m}");
        }
        let mut g = fixture_for("aarch64-pc-windows-msvc");
        g.project.msix_publisher = Some("CN=Ann & Co, O=Ann, C=US".into());
        let m = manifest(&g.ctx()).unwrap();
        assert!(
            m.contains("ProcessorArchitecture=\"arm64\"")
                && m.contains("CN=Ann &amp; Co, O=Ann, C=US")
        );
        let mut h = fixture_for("x86_64-pc-windows-msvc");
        h.project.version = "nightly".into();
        assert!(manifest(&h.ctx()).is_err());
    }

    #[test]
    fn block_map_hashes_every_64k_block() {
        let data = vec![0xABu8; BLOCK * 2 + 5];
        let bm = block_map(&[
            ("Assets/a.png".into(), &data, 42),
            ("e.exe".into(), b"", 36),
        ]);
        assert!(bm.contains("Name=\"Assets\\a.png\" Size=\"131077\" LfhSize=\"42\""));
        assert_eq!(
            bm.matches("<Block ").count(),
            3,
            "two full blocks plus the tail; empty file has none"
        );
        assert!(bm.contains(&base64(&sha256(&data[..BLOCK]))));
        assert!(bm.contains(&base64(&sha256(&data[BLOCK * 2..]))));
    }

    #[test]
    fn builds_a_package_with_the_expected_entries() {
        let mut f = fixture_for("x86_64-pc-windows-msvc");
        f.with_icons = true;
        let bytes = build(&f.ctx()).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let text = String::from_utf8_lossy(&bytes);
        for name in [
            "[Content_Types].xml",
            "AppxManifest.xml",
            "AppxBlockMap.xml",
            "demo.exe",
            "Assets/StoreLogo.png",
            "Assets/Square44x44Logo.png",
            "Assets/Square150x150Logo.png",
            "alloy.capabilities.toml",
        ] {
            assert!(text.contains(name), "missing {name}");
        }
        assert_eq!(bytes, build(&f.ctx()).unwrap(), "reproducible");
    }

    #[test]
    fn needs_icon_and_windows_target() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        assert!(build(&f.ctx()).is_err());
        let l = fixture();
        assert!(msix(&l.ctx()).is_err());
    }
}

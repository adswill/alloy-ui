//! RPM package writer (format version 3, gzip cpio payload).
//!
//! Written from the RPM file format description: a 96-byte lead, a signature
//! header, the main header and a compressed `cpio` (newc) payload. Headers are
//! index-plus-data-store blobs with an "immutable region" trailer. The
//! signature header carries the header's SHA-256 and the sizes; packages are
//! unsigned.

use super::linux::{desktop_entry, require_linux, share_dir};
use super::sha256::sha256_hex;
use super::{write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use flate2::{Compression, GzBuilder};
use std::io::Write;
use std::path::PathBuf;

// Tag numbers (rpmtag.h).
const HEADERSIGNATURES: u32 = 62;
const HEADERIMMUTABLE: u32 = 63;
const HEADERI18NTABLE: u32 = 100;
const SIG_SIZE: u32 = 1000;
const SIG_PAYLOADSIZE: u32 = 1007;
const SIG_SHA256: u32 = 273;
const NAME: u32 = 1000;
const VERSION: u32 = 1001;
const RELEASE: u32 = 1002;
const SUMMARY: u32 = 1004;
const DESCRIPTION: u32 = 1005;
const BUILDTIME: u32 = 1006;
const BUILDHOST: u32 = 1007;
const SIZE: u32 = 1009;
const LICENSE: u32 = 1014;
const PACKAGER: u32 = 1015;
const GROUP: u32 = 1016;
const URL: u32 = 1020;
const OS: u32 = 1021;
const ARCH: u32 = 1022;
const FILESIZES: u32 = 1028;
const FILEMODES: u32 = 1030;
const FILERDEVS: u32 = 1033;
const FILEMTIMES: u32 = 1034;
const FILEDIGESTS: u32 = 1035;
const FILELINKTOS: u32 = 1036;
const FILEFLAGS: u32 = 1037;
const FILEUSERNAME: u32 = 1039;
const FILEGROUPNAME: u32 = 1040;
const REQUIREFLAGS: u32 = 1048;
const REQUIRENAME: u32 = 1049;
const REQUIREVERSION: u32 = 1050;
const FILEDEVICES: u32 = 1095;
const FILEINODES: u32 = 1096;
const FILELANGS: u32 = 1097;
const DIRINDEXES: u32 = 1116;
const BASENAMES: u32 = 1117;
const DIRNAMES: u32 = 1118;
const PAYLOADFORMAT: u32 = 1124;
const PAYLOADCOMPRESSOR: u32 = 1125;
const PAYLOADFLAGS: u32 = 1126;
const FILEDIGESTALGO: u32 = 5011;
const PAYLOADDIGEST: u32 = 5092;
const PAYLOADDIGESTALGO: u32 = 5093;
const PAYLOADDIGESTALT: u32 = 5333;

const SENSE_LESS_EQ: u32 = 0x02 | 0x08;
const SENSE_RPMLIB: u32 = 0x0100_0000;
const SENSE_ANY: u32 = 0;
const DIGEST_SHA256: u32 = 8;

/// A header tag value.
#[derive(Debug, Clone)]
enum Val {
    Str(String),
    I18n(String),
    StrArray(Vec<String>),
    Int16(Vec<u16>),
    Int32(Vec<u32>),
}

impl Val {
    fn type_id(&self) -> u32 {
        match self {
            Val::Str(_) => 6,
            Val::I18n(_) => 9,
            Val::StrArray(_) => 8,
            Val::Int16(_) => 3,
            Val::Int32(_) => 4,
        }
    }
    fn count(&self) -> u32 {
        match self {
            Val::Str(_) | Val::I18n(_) => 1,
            Val::StrArray(v) => v.len() as u32,
            Val::Int16(v) => v.len() as u32,
            Val::Int32(v) => v.len() as u32,
        }
    }
    fn align(&self) -> usize {
        match self {
            Val::Int16(_) => 2,
            Val::Int32(_) => 4,
            _ => 1,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match self {
            Val::Str(s) | Val::I18n(s) => {
                b.extend_from_slice(s.as_bytes());
                b.push(0);
            }
            Val::StrArray(v) => {
                for s in v {
                    b.extend_from_slice(s.as_bytes());
                    b.push(0);
                }
            }
            Val::Int16(v) => v.iter().for_each(|x| b.extend_from_slice(&x.to_be_bytes())),
            Val::Int32(v) => v.iter().for_each(|x| b.extend_from_slice(&x.to_be_bytes())),
        }
        b
    }
}

/// Serialise a header blob: magic, index, data store, with a region trailer
/// under `region_tag` (62 for the signature header, 63 for the main one).
fn header(mut tags: Vec<(u32, Val)>, region_tag: u32) -> Vec<u8> {
    tags.sort_by_key(|t| t.0);
    let mut data: Vec<u8> = Vec::new();
    let mut index: Vec<(u32, u32, u32, u32)> = Vec::new();
    for (tag, v) in &tags {
        while data.len() % v.align() != 0 {
            data.push(0);
        }
        index.push((*tag, v.type_id(), data.len() as u32, v.count()));
        data.extend_from_slice(&v.bytes());
    }
    // Region trailer: an index-entry-shaped record at the end of the store.
    let il = index.len() as u32 + 1; // including the region entry itself
    let trailer_off = data.len() as u32;
    data.extend_from_slice(&region_tag.to_be_bytes());
    data.extend_from_slice(&7u32.to_be_bytes());
    data.extend_from_slice(&(-((il * 16) as i32)).to_be_bytes());
    data.extend_from_slice(&16u32.to_be_bytes());
    let mut out = vec![0x8e, 0xad, 0xe8, 0x01, 0, 0, 0, 0];
    out.extend_from_slice(&il.to_be_bytes());
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&region_tag.to_be_bytes());
    out.extend_from_slice(&7u32.to_be_bytes());
    out.extend_from_slice(&trailer_off.to_be_bytes());
    out.extend_from_slice(&16u32.to_be_bytes());
    for (tag, ty, off, count) in index {
        for x in [tag, ty, off, count] {
            out.extend_from_slice(&x.to_be_bytes());
        }
    }
    out.extend_from_slice(&data);
    out
}

/// A file to package.
struct Entry {
    path: String, // absolute install path
    data: Vec<u8>,
    mode: u16, // includes the S_IFREG bit
}

/// `cpio` newc archive of `entries`, names prefixed with `./`.
fn cpio(entries: &[Entry], mtime: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut put = |ino: u32, mode: u32, size: u32, name: &str, data: &[u8]| {
        let namesize = name.len() as u32 + 1;
        out.extend_from_slice(b"070701");
        for f in [ino, mode, 0, 0, 1, mtime, size, 0, 0, 0, 0, namesize, 0] {
            out.extend_from_slice(format!("{f:08x}").as_bytes());
        }
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    };
    for (i, e) in entries.iter().enumerate() {
        put(
            i as u32 + 1,
            e.mode as u32,
            e.data.len() as u32,
            &format!(".{}", e.path),
            &e.data,
        );
    }
    put(0, 0, 0, "TRAILER!!!", &[]);
    out
}

/// RPM architecture name for a Rust arch.
pub fn rpm_arch(arch: &str) -> Result<&'static str> {
    Ok(match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "i686" | "i586" => "i686",
        a if a.starts_with("armv7") => "armv7hl",
        "riscv64gc" | "riscv64" => "riscv64",
        other => {
            return Err(Error::Config(format!(
                "no RPM architecture known for `{other}`"
            )))
        }
    })
}

/// RPM version: `-` is reserved, so pre-release markers become `~`.
pub fn rpm_version(v: &str) -> String {
    v.replace('-', "~")
}

fn build_entries(ctx: &Ctx<'_>) -> Result<Vec<Entry>> {
    let p = ctx.project;
    let bin = &p.bin_name;
    let mut e = vec![Entry {
        path: format!("/usr/bin/{bin}"),
        data: ctx.binary_bytes()?,
        mode: 0o100755,
    }];
    let mut file = |path: String, data: Vec<u8>| {
        e.push(Entry {
            path,
            data,
            mode: 0o100644,
        })
    };
    file(
        format!("/usr/share/applications/{}.desktop", p.identifier),
        desktop_entry(ctx).into_bytes(),
    );
    for i in &ctx.icons.pngs {
        file(
            format!("/usr/share/icons/hicolor/{0}x{0}/apps/{bin}.png", i.size),
            i.data.clone(),
        );
    }
    if let Some(c) = &ctx.capabilities {
        file(
            format!("/{}/{CAPABILITIES_FILE}", share_dir(ctx)),
            c.clone().into_bytes(),
        );
    }
    e.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(e)
}

/// Assemble the complete `.rpm` file bytes.
pub fn build(ctx: &Ctx<'_>) -> Result<Vec<u8>> {
    let p = ctx.project;
    let name = super::deb::package_name(&p.bin_name)?;
    let version = rpm_version(&p.version);
    let release = "1";
    let arch = rpm_arch(&ctx.arch)?;
    let entries = build_entries(ctx)?;
    let mtime = ctx.mtime() as u32;

    // Split paths into directory + basename tables.
    let mut dirs: Vec<String> = Vec::new();
    let (mut dirindex, mut basenames) = (Vec::new(), Vec::new());
    for e in &entries {
        let (d, b) = e.path.rsplit_once('/').expect("absolute path");
        let d = format!("{d}/");
        let i = dirs.iter().position(|x| *x == d).unwrap_or_else(|| {
            dirs.push(d.clone());
            dirs.len() - 1
        });
        dirindex.push(i as u32);
        basenames.push(b.to_string());
    }
    let n = entries.len();
    let total: u64 = entries.iter().map(|e| e.data.len() as u64).sum();

    let mut requires: Vec<(String, String, u32)> = vec![
        (
            "rpmlib(CompressedFileNames)".into(),
            "3.0.4-1".into(),
            SENSE_RPMLIB | SENSE_LESS_EQ,
        ),
        (
            "rpmlib(PayloadFilesHavePrefix)".into(),
            "4.0-1".into(),
            SENSE_RPMLIB | SENSE_LESS_EQ,
        ),
    ];
    requires.extend(
        p.rpm_requires
            .iter()
            .map(|r| (r.clone(), String::new(), SENSE_ANY)),
    );
    let summary = p
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let mut tags = vec![
        (HEADERI18NTABLE, Val::StrArray(vec!["C".into()])),
        (NAME, Val::Str(name.clone())),
        (VERSION, Val::Str(version.clone())),
        (RELEASE, Val::Str(release.into())),
        (SUMMARY, Val::I18n(summary.clone())),
        (DESCRIPTION, Val::I18n(summary)),
        (BUILDTIME, Val::Int32(vec![mtime])),
        (BUILDHOST, Val::Str("localhost".into())),
        (SIZE, Val::Int32(vec![total as u32])),
        (
            LICENSE,
            Val::Str(p.license.clone().unwrap_or_else(|| "Proprietary".into())),
        ),
        (GROUP, Val::I18n("Unspecified".into())),
        (OS, Val::Str("linux".into())),
        (ARCH, Val::Str(arch.into())),
        (
            FILESIZES,
            Val::Int32(entries.iter().map(|e| e.data.len() as u32).collect()),
        ),
        (
            FILEMODES,
            Val::Int16(entries.iter().map(|e| e.mode).collect()),
        ),
        (FILERDEVS, Val::Int16(vec![0; n])),
        (FILEMTIMES, Val::Int32(vec![mtime; n])),
        (
            FILEDIGESTS,
            Val::StrArray(entries.iter().map(|e| sha256_hex(&e.data)).collect()),
        ),
        (FILELINKTOS, Val::StrArray(vec![String::new(); n])),
        (FILEFLAGS, Val::Int32(vec![0; n])),
        (FILEUSERNAME, Val::StrArray(vec!["root".into(); n])),
        (FILEGROUPNAME, Val::StrArray(vec!["root".into(); n])),
        (
            REQUIREFLAGS,
            Val::Int32(requires.iter().map(|r| r.2).collect()),
        ),
        (
            REQUIRENAME,
            Val::StrArray(requires.iter().map(|r| r.0.clone()).collect()),
        ),
        (
            REQUIREVERSION,
            Val::StrArray(requires.iter().map(|r| r.1.clone()).collect()),
        ),
        (FILEDEVICES, Val::Int32(vec![1; n])),
        (FILEINODES, Val::Int32((1..=n as u32).collect())),
        (FILELANGS, Val::StrArray(vec![String::new(); n])),
        (DIRINDEXES, Val::Int32(dirindex)),
        (BASENAMES, Val::StrArray(basenames)),
        (DIRNAMES, Val::StrArray(dirs)),
        (PAYLOADFORMAT, Val::Str("cpio".into())),
        (PAYLOADCOMPRESSOR, Val::Str("gzip".into())),
        (PAYLOADFLAGS, Val::Str("9".into())),
        (FILEDIGESTALGO, Val::Int32(vec![DIGEST_SHA256])),
    ];
    if let Some(h) = &p.homepage {
        tags.push((URL, Val::Str(h.clone())));
    }
    let packager = p.authors.first().cloned().or_else(|| p.publisher.clone());
    if let Some(pk) = packager {
        tags.push((PACKAGER, Val::Str(pk)));
    }
    let raw_cpio = cpio(&entries, mtime);
    let mut gz = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    gz.write_all(&raw_cpio)
        .map_err(crate::error::io("compressing rpm payload"))?;
    let payload = gz
        .finish()
        .map_err(crate::error::io("compressing rpm payload"))?;

    tags.push((PAYLOADDIGEST, Val::StrArray(vec![sha256_hex(&payload)])));
    tags.push((PAYLOADDIGESTALGO, Val::Int32(vec![DIGEST_SHA256])));
    tags.push((PAYLOADDIGESTALT, Val::StrArray(vec![sha256_hex(&raw_cpio)])));
    let main = header(tags, HEADERIMMUTABLE);

    let sig = header(
        vec![
            (
                SIG_SIZE,
                Val::Int32(vec![(main.len() + payload.len()) as u32]),
            ),
            (SIG_SHA256, Val::Str(sha256_hex(&main))),
            (SIG_PAYLOADSIZE, Val::Int32(vec![raw_cpio.len() as u32])),
        ],
        HEADERSIGNATURES,
    );

    let mut out = Vec::new();
    // Lead.
    out.extend_from_slice(&[0xed, 0xab, 0xee, 0xdb, 3, 0]);
    out.extend_from_slice(&0u16.to_be_bytes()); // binary package
    out.extend_from_slice(&0u16.to_be_bytes()); // archnum (the header is authoritative)
    let mut nm = format!("{name}-{version}-{release}").into_bytes();
    nm.resize(66, 0);
    out.extend_from_slice(&nm[..66]);
    out.extend_from_slice(&1u16.to_be_bytes()); // osnum: linux
    out.extend_from_slice(&5u16.to_be_bytes()); // signature type: header-style
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&sig);
    while out.len() % 8 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&main);
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Build the `.rpm`, returning its path.
pub fn rpm(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_linux(ctx, "rpm")?;
    let p = ctx.project;
    let out = ctx.out_dir.join(format!(
        "{}-{}-1.{}.rpm",
        super::deb::package_name(&p.bin_name)?,
        rpm_version(&p.version),
        rpm_arch(&ctx.arch)?
    ));
    write_file(&out, &build(ctx)?)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::fixture;

    #[test]
    fn arch_and_version() {
        assert_eq!(rpm_arch("aarch64").unwrap(), "aarch64");
        assert!(rpm_arch("sparc").is_err());
        assert_eq!(rpm_version("1.2.3-beta.1"), "1.2.3~beta.1");
    }

    #[test]
    fn header_layout() {
        let h = header(
            vec![(NAME, Val::Str("x".into())), (SIZE, Val::Int32(vec![7]))],
            HEADERIMMUTABLE,
        );
        assert_eq!(&h[..8], &[0x8e, 0xad, 0xe8, 0x01, 0, 0, 0, 0]);
        let il = u32::from_be_bytes(h[8..12].try_into().unwrap());
        let dl = u32::from_be_bytes(h[12..16].try_into().unwrap());
        assert_eq!(il, 3); // two tags + region
        assert_eq!(h.len(), 16 + 16 * il as usize + dl as usize);
        // Region entry sorts first; 32-bit values are 4-byte aligned in the store.
        assert_eq!(
            u32::from_be_bytes(h[16..20].try_into().unwrap()),
            HEADERIMMUTABLE
        );
        let size_entry = &h[16 + 32..16 + 48];
        assert_eq!(
            u32::from_be_bytes(size_entry[0..4].try_into().unwrap()),
            SIZE
        );
        assert_eq!(
            u32::from_be_bytes(size_entry[8..12].try_into().unwrap()) % 4,
            0
        );
    }

    #[test]
    fn cpio_is_aligned_and_terminated() {
        let c = cpio(
            &[Entry {
                path: "/a/b".into(),
                data: b"xyz".to_vec(),
                mode: 0o100644,
            }],
            0,
        );
        assert!(c.starts_with(b"070701"));
        assert_eq!(c.len() % 4, 0);
        assert!(c.windows(10).any(|w| w == b"TRAILER!!!"));
        assert!(c.windows(5).any(|w| w == b"./a/b"));
    }

    #[test]
    fn package_has_lead_and_two_headers() {
        let f = fixture();
        let b = build(&f.ctx()).unwrap();
        assert_eq!(&b[..4], &[0xed, 0xab, 0xee, 0xdb]);
        assert_eq!(&b[96..100], &[0x8e, 0xad, 0xe8, 0x01]);
        assert!(
            b.windows(2).any(|w| w == [0x1f, 0x8b]),
            "gzip payload present"
        );
    }

    #[test]
    fn build_is_reproducible() {
        let f = fixture();
        assert_eq!(build(&f.ctx()).unwrap(), build(&f.ctx()).unwrap());
    }
}

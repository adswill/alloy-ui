//! Minimal archive writers for `.deb`: a reproducible tar.gz builder and an
//! `ar` container.

use crate::error::{io, Result};
use flate2::{Compression, GzBuilder};
use std::collections::BTreeSet;

/// Builds a gzip-compressed tar with root ownership and a fixed mtime.
pub struct TarGz {
    builder: tar::Builder<flate2::write::GzEncoder<Vec<u8>>>,
    mtime: u64,
    dirs: BTreeSet<String>,
    /// Total bytes of file payloads added.
    pub payload: u64,
}

impl TarGz {
    /// Start an empty archive.
    pub fn new(mtime: u64) -> Self {
        let gz = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::best());
        Self {
            builder: tar::Builder::new(gz),
            mtime,
            dirs: BTreeSet::new(),
            payload: 0,
        }
    }

    fn header(&self, mode: u32, size: u64, kind: tar::EntryType) -> tar::Header {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(kind);
        h.set_mode(mode);
        h.set_size(size);
        h.set_mtime(self.mtime);
        h.set_uid(0);
        h.set_gid(0);
        let _ = h.set_username("root");
        let _ = h.set_groupname("root");
        h
    }

    /// Append an entry, keeping the leading `./` that Debian tools expect
    /// (the `tar` crate's `set_path` would strip it).
    fn append(&mut self, mut h: tar::Header, name: &str, data: &[u8]) -> std::io::Result<()> {
        if name.len() < 100 {
            let field = &mut h.as_old_mut().name;
            field.fill(0);
            field[..name.len()].copy_from_slice(name.as_bytes());
            h.set_cksum();
            self.builder.append(&h, data)
        } else {
            // Long names need a GNU extension record; `append_data` writes it.
            self.builder.append_data(&mut h, name, data)
        }
    }

    /// Add a directory entry (and its parents) if not already present.
    pub fn dir(&mut self, path: &str) -> Result<()> {
        let path = path.trim_matches('/');
        let mut acc = String::new();
        for part in path.split('/').filter(|p| !p.is_empty() && *p != ".") {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            if self.dirs.insert(acc.clone()) {
                let h = self.header(0o755, 0, tar::EntryType::Directory);
                self.append(h, &format!("./{acc}/"), &[])
                    .map_err(io(format!("tar dir {acc}")))?;
            }
        }
        Ok(())
    }

    /// Add a file, creating parent directories first.
    pub fn file(&mut self, path: &str, data: &[u8], mode: u32) -> Result<()> {
        if let Some((parent, _)) = path.rsplit_once('/') {
            self.dir(parent)?;
        }
        let h = self.header(mode, data.len() as u64, tar::EntryType::Regular);
        self.append(h, &format!("./{}", path.trim_start_matches('/')), data)
            .map_err(io(format!("tar file {path}")))?;
        self.payload += data.len() as u64;
        Ok(())
    }

    /// Finish and return the compressed bytes.
    pub fn finish(self) -> Result<Vec<u8>> {
        let gz = self.builder.into_inner().map_err(io("finishing tar"))?;
        gz.finish().map_err(io("finishing gzip"))
    }
}

/// Build an `ar` archive (the `.deb` outer container).
pub fn ar(members: &[(&str, &[u8])], mtime: u64) -> Vec<u8> {
    let mut out = b"!<arch>\n".to_vec();
    for (name, data) in members {
        let header = format!(
            "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
            name,
            mtime,
            0,
            0,
            "100644",
            data.len()
        );
        debug_assert_eq!(header.len(), 60);
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn ar_layout_and_padding() {
        let a = ar(&[("a", b"xyz"), ("b.tar", b"12")], 7);
        assert_eq!(&a[..8], b"!<arch>\n");
        assert_eq!(&a[8 + 58..8 + 60], b"`\n");
        let first = std::str::from_utf8(&a[8..8 + 60]).unwrap();
        assert!(first.starts_with("a               7 "));
        assert!(first.contains("100644  3         `"));
        // 3-byte payload is padded to even length.
        assert_eq!(&a[68..72], b"xyz\n");
        assert_eq!(a.len(), 8 + 60 + 4 + 60 + 2);
    }

    #[test]
    fn tar_roundtrip_with_parents() {
        let mut t = TarGz::new(0);
        t.file("usr/bin/app", b"binary", 0o755).unwrap();
        t.file("usr/share/app/x.toml", b"cfg", 0o644).unwrap();
        assert_eq!(t.payload, 9);
        let gz = t.finish().unwrap();
        let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(&gz[..]));
        let mut seen = vec![];
        for e in ar.entries().unwrap() {
            let mut e = e.unwrap();
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            seen.push((
                e.path().unwrap().to_string_lossy().into_owned(),
                e.header().mode().unwrap(),
                e.header().uid().unwrap(),
                s,
            ));
        }
        let names: Vec<&str> = seen.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "./usr/",
                "./usr/bin/",
                "./usr/bin/app",
                "./usr/share/",
                "./usr/share/app/",
                "./usr/share/app/x.toml"
            ]
        );
        let app = seen.iter().find(|s| s.0 == "./usr/bin/app").unwrap();
        assert_eq!((app.1, app.2, app.3.as_str()), (0o755, 0, "binary"));
    }

    #[test]
    fn tar_is_reproducible() {
        let make = || {
            let mut t = TarGz::new(0);
            t.file("a/b", b"1", 0o644).unwrap();
            t.finish().unwrap()
        };
        assert_eq!(make(), make());
    }
}

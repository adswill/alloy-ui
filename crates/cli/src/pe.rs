//! Embedding an application icon into a Windows PE executable.
//!
//! Rust's MSVC and GNU targets link executables without a resource section,
//! so the icon Explorer shows is the generic one. This module appends a
//! `.rsrc` section holding `RT_ICON` and `RT_GROUP_ICON` resources, the same
//! layout `rc.exe` would produce, without needing any Windows tooling.
//!
//! Only the simple case is handled: the file must have no existing resource
//! directory, no data after the last section (certificates, overlays) and
//! free room in the header area for one more section entry. Otherwise an
//! error is returned and the caller keeps the original executable.

use crate::icon::IcoImage;

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const LANG_EN_US: u32 = 0x0409;

/// Why an icon could not be embedded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PeError {
    /// Not a PE file, or the headers are truncated.
    #[error("not a valid PE file: {0}")]
    Invalid(&'static str),
    /// The executable already has resources.
    #[error("executable already has a resource section")]
    HasResources,
    /// Data follows the last section.
    #[error("executable has data after its last section (overlay or signature)")]
    Overlay,
    /// No room for another section header.
    #[error("no room in the PE header for another section")]
    NoHeaderRoom,
    /// No icon images were provided.
    #[error("no icon images")]
    NoImages,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, PeError> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(PeError::Invalid("truncated"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, PeError> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(PeError::Invalid("truncated"))
}

fn put_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn align(v: u32, a: u32) -> u32 {
    v.div_ceil(a) * a
}

// ---- resource tree -------------------------------------------------------

/// Builds the resource directory tree for the icon resources.
/// Layout: type dir -> id dir -> language dir -> data entry, then blobs.
fn build_rsrc(images: &[IcoImage], section_rva: u32) -> Vec<u8> {
    // (type, id, bytes) leaves, sorted by type then id as the format requires.
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut leaves: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        let id = i as u32 + 1;
        group.extend_from_slice(&[img.width, img.height, img.colors, 0]);
        group.extend_from_slice(&img.planes.to_le_bytes());
        group.extend_from_slice(&img.bpp.to_le_bytes());
        group.extend_from_slice(&(img.data.len() as u32).to_le_bytes());
        group.extend_from_slice(&(id as u16).to_le_bytes());
        leaves.push((RT_ICON, id, img.data.clone()));
    }
    leaves.push((RT_GROUP_ICON, 1, group));

    // Group leaves by type (already ascending: 3 then 14).
    let mut types: Vec<u32> = leaves.iter().map(|l| l.0).collect();
    types.dedup();

    let dir_size = |entries: usize| 16 + 8 * entries;
    // Pass 1: compute offsets.
    let root_off = 0usize;
    let mut off = dir_size(types.len());
    let mut type_dir_off = Vec::new(); // per type: offset of its id directory
    for t in &types {
        type_dir_off.push(off);
        off += dir_size(leaves.iter().filter(|l| l.0 == *t).count());
    }
    let mut lang_dir_off = Vec::new(); // per leaf
    for _ in &leaves {
        lang_dir_off.push(off);
        off += dir_size(1);
    }
    let data_entry_off: Vec<usize> = (0..leaves.len()).map(|i| off + i * 16).collect();
    off += leaves.len() * 16;
    let mut blob_off = Vec::new();
    for l in &leaves {
        off = off.div_ceil(4) * 4;
        blob_off.push(off);
        off += l.2.len();
    }
    let total = off;

    // Pass 2: write.
    let mut out = vec![0u8; total];
    let dir = |out: &mut Vec<u8>, at: usize, entries: &[(u32, u32)]| {
        // characteristics, timestamp, versions = 0; then named=0, id count.
        put_u16(out, at + 14, entries.len() as u16);
        for (i, (id, target)) in entries.iter().enumerate() {
            put_u32(out, at + 16 + i * 8, *id);
            put_u32(out, at + 16 + i * 8 + 4, *target);
        }
    };
    let subdir_flag = 0x8000_0000u32;
    let root: Vec<(u32, u32)> = types
        .iter()
        .zip(&type_dir_off)
        .map(|(t, o)| (*t, *o as u32 | subdir_flag))
        .collect();
    dir(&mut out, root_off, &root);
    for (ti, t) in types.iter().enumerate() {
        let entries: Vec<(u32, u32)> = leaves
            .iter()
            .enumerate()
            .filter(|(_, l)| l.0 == *t)
            .map(|(i, l)| (l.1, lang_dir_off[i] as u32 | subdir_flag))
            .collect();
        dir(&mut out, type_dir_off[ti], &entries);
    }
    for i in 0..leaves.len() {
        dir(
            &mut out,
            lang_dir_off[i],
            &[(LANG_EN_US, data_entry_off[i] as u32)],
        );
        let e = data_entry_off[i];
        put_u32(&mut out, e, section_rva + blob_off[i] as u32);
        put_u32(&mut out, e + 4, leaves[i].2.len() as u32);
        put_u32(&mut out, e + 8, 0); // code page
        out[blob_off[i]..blob_off[i] + leaves[i].2.len()].copy_from_slice(&leaves[i].2);
    }
    out
}

/// Return a copy of the PE image `exe` with `images` embedded as its icon.
pub fn embed_icon(exe: &[u8], images: &[IcoImage]) -> Result<Vec<u8>, PeError> {
    if images.is_empty() {
        return Err(PeError::NoImages);
    }
    if exe.get(..2) != Some(b"MZ") {
        return Err(PeError::Invalid("missing MZ header"));
    }
    let pe = u32_at(exe, 0x3c)? as usize;
    if exe.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(PeError::Invalid("missing PE signature"));
    }
    let coff = pe + 4;
    let nsec = u16_at(exe, coff + 2)? as usize;
    let opt_size = u16_at(exe, coff + 16)? as usize;
    let opt = coff + 20;
    let dd_off = match u16_at(exe, opt)? {
        0x10b => opt + 96,
        0x20b => opt + 112,
        _ => return Err(PeError::Invalid("unknown optional header magic")),
    };
    let (sec_align, file_align) = (u32_at(exe, opt + 32)?, u32_at(exe, opt + 36)?);
    if sec_align == 0 || file_align == 0 {
        return Err(PeError::Invalid("zero alignment"));
    }
    let ndd = u32_at(exe, dd_off - 4)? as usize;
    if ndd < 3 {
        return Err(PeError::Invalid("no resource data directory slot"));
    }
    if u32_at(exe, dd_off + 16)? != 0 || u32_at(exe, dd_off + 20)? != 0 {
        return Err(PeError::HasResources);
    }
    let table = opt + opt_size;
    let table_end = table + nsec * 40;
    let (mut max_rva, mut max_raw, mut first_raw) = (0u32, 0u32, u32::MAX);
    for i in 0..nsec {
        let s = table + i * 40;
        let (vsize, va) = (u32_at(exe, s + 8)?, u32_at(exe, s + 12)?);
        let (rsize, rptr) = (u32_at(exe, s + 16)?, u32_at(exe, s + 20)?);
        max_rva = max_rva.max(va + vsize.max(rsize));
        if rptr != 0 {
            max_raw = max_raw.max(rptr + rsize);
            first_raw = first_raw.min(rptr);
        }
    }
    if exe.len() as u32 > max_raw {
        return Err(PeError::Overlay);
    }
    if first_raw != u32::MAX && (first_raw as usize) < table_end + 40 {
        return Err(PeError::NoHeaderRoom);
    }
    if table_end + 40 > exe.len() {
        return Err(PeError::Invalid("truncated section table"));
    }

    let rva = align(max_rva, sec_align);
    let rsrc = build_rsrc(images, rva);
    let vsize = rsrc.len() as u32;
    let raw_size = align(vsize, file_align);
    let raw_ptr = align(max_raw, file_align);

    let mut out = exe.to_vec();
    out.resize(raw_ptr as usize, 0);
    out.extend_from_slice(&rsrc);
    out.resize((raw_ptr + raw_size) as usize, 0);

    // New section header.
    let s = table_end;
    out[s..s + 8].copy_from_slice(b".rsrc\0\0\0");
    put_u32(&mut out, s + 8, vsize);
    put_u32(&mut out, s + 12, rva);
    put_u32(&mut out, s + 16, raw_size);
    put_u32(&mut out, s + 20, raw_ptr);
    // Relocations/line numbers stay zero. Initialized data | readable.
    put_u32(&mut out, s + 36, 0x4000_0040);

    put_u16(&mut out, coff + 2, nsec as u16 + 1);
    let init_data = u32_at(&out, opt + 8)?;
    put_u32(&mut out, opt + 8, init_data + raw_size);
    put_u32(&mut out, opt + 56, align(rva + vsize, sec_align));
    put_u32(&mut out, opt + 64, 0); // checksum: not validated for user-mode exes
    put_u32(&mut out, dd_off + 16, rva);
    put_u32(&mut out, dd_off + 20, vsize);
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::icon::{parse_ico, test_png, IconSet, PngIcon};

    /// A minimal but structurally valid PE32+ with one `.text` section.
    pub(crate) fn tiny_pe() -> Vec<u8> {
        let mut b = vec![0u8; 0x400];
        b[..2].copy_from_slice(b"MZ");
        put_u32(&mut b, 0x3c, 0x80);
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        put_u16(&mut b, coff, 0x8664);
        put_u16(&mut b, coff + 2, 1);
        put_u16(&mut b, coff + 16, 240); // optional header size (PE32+ w/ 16 dirs)
        let opt = coff + 20;
        put_u16(&mut b, opt, 0x20b);
        put_u32(&mut b, opt + 8, 0x200); // SizeOfInitializedData
        put_u32(&mut b, opt + 32, 0x1000);
        put_u32(&mut b, opt + 36, 0x200);
        put_u32(&mut b, opt + 56, 0x2000);
        put_u32(&mut b, opt + 60, 0x200);
        put_u32(&mut b, opt + 108, 16);
        let sec = opt + 240;
        b[sec..sec + 5].copy_from_slice(b".text");
        put_u32(&mut b, sec + 8, 0x100);
        put_u32(&mut b, sec + 12, 0x1000);
        put_u32(&mut b, sec + 16, 0x200);
        put_u32(&mut b, sec + 20, 0x200);
        put_u32(&mut b, sec + 36, 0x6000_0020);
        b[0x200] = 0xc3;
        b.truncate(0x400);
        b
    }

    fn images() -> Vec<IcoImage> {
        let set = IconSet {
            pngs: [32u32, 256]
                .iter()
                .map(|&s| PngIcon {
                    size: s,
                    data: test_png(s),
                })
                .collect(),
            ..Default::default()
        };
        parse_ico(&set.ico_bytes().unwrap()).unwrap()
    }

    /// Walk the resource tree like the Windows loader would and return
    /// `(type, id, bytes)` for every leaf.
    pub(crate) fn read_resources(pe_bytes: &[u8]) -> Vec<(u32, u32, Vec<u8>)> {
        let pe = u32_at(pe_bytes, 0x3c).unwrap() as usize;
        let coff = pe + 4;
        let nsec = u16_at(pe_bytes, coff + 2).unwrap() as usize;
        let opt = coff + 20;
        let dd = if u16_at(pe_bytes, opt).unwrap() == 0x20b {
            opt + 112
        } else {
            opt + 96
        };
        let (rva, size) = (
            u32_at(pe_bytes, dd + 16).unwrap(),
            u32_at(pe_bytes, dd + 20).unwrap(),
        );
        assert!(size > 0);
        let table = opt + u16_at(pe_bytes, coff + 16).unwrap() as usize;
        let rva_to_off = |r: u32| -> usize {
            for i in 0..nsec {
                let s = table + i * 40;
                let (v, va, rs, rp) = (
                    u32_at(pe_bytes, s + 8).unwrap(),
                    u32_at(pe_bytes, s + 12).unwrap(),
                    u32_at(pe_bytes, s + 16).unwrap(),
                    u32_at(pe_bytes, s + 20).unwrap(),
                );
                if r >= va && r < va + v.max(rs) {
                    return (rp + (r - va)) as usize;
                }
            }
            panic!("rva {r:#x} not in any section");
        };
        let base = rva_to_off(rva);
        let entries = |at: usize| -> Vec<(u32, u32)> {
            let n = u16_at(pe_bytes, base + at + 12).unwrap() as usize
                + u16_at(pe_bytes, base + at + 14).unwrap() as usize;
            (0..n)
                .map(|i| {
                    (
                        u32_at(pe_bytes, base + at + 16 + i * 8).unwrap(),
                        u32_at(pe_bytes, base + at + 20 + i * 8).unwrap(),
                    )
                })
                .collect()
        };
        let mut out = Vec::new();
        for (t, to) in entries(0) {
            assert!(to & 0x8000_0000 != 0);
            for (id, io) in entries((to & 0x7fff_ffff) as usize) {
                for (_lang, lo) in entries((io & 0x7fff_ffff) as usize) {
                    assert!(lo & 0x8000_0000 == 0, "leaf expected");
                    let de = base + lo as usize;
                    let (data_rva, len) = (
                        u32_at(pe_bytes, de).unwrap(),
                        u32_at(pe_bytes, de + 4).unwrap(),
                    );
                    let o = rva_to_off(data_rva);
                    out.push((t, id, pe_bytes[o..o + len as usize].to_vec()));
                }
            }
        }
        out
    }

    #[test]
    fn embeds_and_reads_back() {
        let exe = tiny_pe();
        let imgs = images();
        let out = embed_icon(&exe, &imgs).unwrap();
        // Original bytes are untouched apart from header fields.
        assert_eq!(&out[0x200..0x201], &[0xc3]);
        let pe = 0x80;
        assert_eq!(u16_at(&out, pe + 4 + 2).unwrap(), 2, "section count");
        let opt = pe + 4 + 20;
        assert_eq!(u32_at(&out, opt + 56).unwrap(), 0x3000, "SizeOfImage");
        assert_eq!(out.len() % 0x200, 0);
        let res = read_resources(&out);
        // Two icons then the group, ordered by type.
        assert_eq!(
            res.iter().map(|r| (r.0, r.1)).collect::<Vec<_>>(),
            vec![(3, 1), (3, 2), (14, 1)]
        );
        assert_eq!(res[0].2, imgs[0].data);
        assert_eq!(res[1].2, imgs[1].data);
        let grp = &res[2].2;
        assert_eq!(&grp[..6], &[0, 0, 1, 0, 2, 0]);
        // Group entry 2 refers to RT_ICON id 2 and the right byte count.
        assert_eq!(u16::from_le_bytes([grp[6 + 14 + 12], grp[6 + 14 + 13]]), 2);
        assert_eq!(
            u32::from_le_bytes(grp[6 + 14 + 8..6 + 14 + 12].try_into().unwrap()) as usize,
            imgs[1].data.len()
        );
    }

    #[test]
    fn section_header_is_sane() {
        let out = embed_icon(&tiny_pe(), &images()).unwrap();
        let s = 0x84 + 20 + 240 + 40;
        assert_eq!(&out[s..s + 5], b".rsrc");
        assert_eq!(u32_at(&out, s + 12).unwrap(), 0x2000); // next section-aligned RVA
        assert_eq!(u32_at(&out, s + 20).unwrap(), 0x400); // appended after .text raw data
        assert_eq!(u32_at(&out, s + 36).unwrap(), 0x4000_0040);
    }

    #[test]
    fn refuses_unsupported_inputs() {
        let imgs = images();
        assert_eq!(
            embed_icon(b"hello", &imgs),
            Err(PeError::Invalid("missing MZ header"))
        );
        assert_eq!(embed_icon(&tiny_pe(), &[]), Err(PeError::NoImages));
        let mut overlay = tiny_pe();
        overlay.extend_from_slice(&[1, 2, 3]);
        assert_eq!(embed_icon(&overlay, &imgs), Err(PeError::Overlay));
        let twice = embed_icon(&tiny_pe(), &imgs).unwrap();
        assert_eq!(embed_icon(&twice, &imgs), Err(PeError::HasResources));
    }

    #[test]
    fn refuses_when_header_is_full() {
        let mut b = tiny_pe();
        // Move the section's raw data to right after the one section header.
        let sec = 0x84 + 20 + 240;
        put_u32(&mut b, sec + 20, (sec + 40) as u32);
        b.truncate(sec + 40 + 0x200); // no overlay
        assert_eq!(embed_icon(&b, &images()), Err(PeError::NoHeaderRoom));
    }
}

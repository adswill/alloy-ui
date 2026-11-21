//! File browser and gallery grid.

use super::*;
use crate::{
    blocks::local,
    choose,
    dsl::{caption, text},
    icons::{icon, Icon},
    view::{col, row},
    Act,
};
use alloy_ui_core::{Color, Rectangle, Size};
use alloy_ui_layout::Length;
use alloy_ui_widgets::Frame;
use std::sync::Arc;

/// What a [`FileEntry`] is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
    /// A directory.
    Folder,
    /// Pictures.
    Image,
    /// Source code.
    Code,
    /// Documents and text.
    Doc,
    /// Anything else.
    Other,
}

/// One row of a [`file_browser`].
#[derive(Clone)]
pub struct FileEntry {
    name: String,
    kind: FileKind,
    size: String,
    modified: String,
}

/// A directory entry: `folder("Designs", "Today")`.
pub fn folder(name: impl Into<String>, modified: impl Into<String>) -> FileEntry {
    FileEntry {
        name: name.into(),
        kind: FileKind::Folder,
        size: String::new(),
        modified: modified.into(),
    }
}

/// A file; the kind comes from the extension: `file("logo.png", "48 KB", "Yesterday")`.
pub fn file(
    name: impl Into<String>,
    size: impl Into<String>,
    modified: impl Into<String>,
) -> FileEntry {
    let name: String = name.into();
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    let kind = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => FileKind::Image,
        "rs" | "toml" | "js" | "ts" | "py" | "json" | "c" | "go" => FileKind::Code,
        "md" | "txt" | "pdf" | "doc" | "docx" => FileKind::Doc,
        _ => FileKind::Other,
    };
    FileEntry {
        name,
        kind,
        size: size.into(),
        modified: modified.into(),
    }
}

const HUES: [u32; 6] = [0x2F6FEB, 0x0E8F7E, 0xC2571A, 0x8B5CF6, 0xC9356B, 0x6B8E23];

/// A generated thumbnail: a tint and a quiet pattern chosen from the name, so every item is
/// recognisable without loading anything.
fn thumb<M: 'static>(name: &str, kind: FileKind, w: f32, h: f32) -> Node<M> {
    let name = name.to_string();
    paint(Length::Fixed(w), h, move |f: &mut Frame| {
        let p = f.palette();
        let seed = hash(&name);
        let hue = Color::hex(HUES[seed as usize % HUES.len()]);
        let r = Rectangle::new(pt(0.0, 0.0), Size::new(w, h));
        f.round_rect(r, 8.0, mix(p.surface, hue, 0.12));
        f.stroke_rect(r, 8.0, 1.0, mix(p.border, hue, 0.2));
        let ink = mix(p.surface, hue, 0.38);
        match kind {
            FileKind::Folder => {
                f.round_rect(
                    Rectangle::new(pt(w * 0.28, h * 0.26), Size::new(w * 0.2, h * 0.14)),
                    4.0,
                    ink,
                );
                f.round_rect(
                    Rectangle::new(pt(w * 0.28, h * 0.34), Size::new(w * 0.44, h * 0.38)),
                    5.0,
                    ink,
                );
            }
            _ => match seed % 3 {
                0 => {
                    f.circle(pt(w * 0.36, h * 0.46), h * 0.24, ink);
                    f.circle(pt(w * 0.62, h * 0.58), h * 0.14, mix(p.surface, hue, 0.24));
                }
                1 => {
                    for i in 0..5 {
                        let bh = h * (0.18 + 0.1 * ((seed >> (i * 3)) % 4) as f32);
                        f.round_rect(
                            Rectangle::new(
                                pt(w * 0.22 + i as f32 * w * 0.12, h * 0.74 - bh),
                                Size::new(w * 0.07, bh),
                            ),
                            2.0,
                            ink,
                        );
                    }
                }
                _ => {
                    for i in 0..4 {
                        f.round_rect(
                            Rectangle::new(
                                pt(w * 0.22, h * 0.28 + i as f32 * h * 0.12),
                                Size::new(w * (0.56 - 0.1 * (i % 3) as f32), h * 0.05),
                            ),
                            2.0,
                            ink,
                        );
                    }
                }
            },
        }
        if kind != FileKind::Folder && kind != FileKind::Image {
            let ext = name.rsplit('.').next().unwrap_or("").to_uppercase();
            if !ext.is_empty() && ext.len() <= 4 && name.contains('.') {
                f.text_weight(
                    pt(8.0, 6.0),
                    ext,
                    10.0,
                    600,
                    mix(p.text_secondary, hue, 0.5),
                );
            }
        }
    })
}

fn glyph(k: FileKind) -> Icon {
    match k {
        FileKind::Folder => Icon::Folder,
        FileKind::Image => Icon::Image,
        FileKind::Code => Icon::Code,
        FileKind::Doc => Icon::FileText,
        FileKind::Other => Icon::File,
    }
}

type OnOpen<S> = Arc<dyn Fn(&mut S, &str) + Send + Sync>;

/// A file browser. See [`file_browser`].
pub struct Files<S> {
    entries: Vec<FileEntry>,
    path: Vec<String>,
    on_open: Option<OnOpen<S>>,
    grid: bool,
}

/// A folder listing with a List/Grid switch, selection and an open action. Folders sort first.
/// `file_browser(entries).path(["Home", "Projects"]).on_open(|s, name| s.open(name))`.
pub fn file_browser<S: 'static>(entries: impl IntoIterator<Item = FileEntry>) -> Files<S> {
    let mut entries: Vec<FileEntry> = entries.into_iter().collect();
    entries.sort_by_key(|e| (e.kind != FileKind::Folder, e.name.to_lowercase()));
    Files {
        entries,
        path: Vec::new(),
        on_open: None,
        grid: false,
    }
}

impl<S: 'static> Files<S> {
    /// The location shown above the list.
    pub fn path(mut self, p: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.path = p.into_iter().map(Into::into).collect();
        self
    }
    /// Called with the name when an already selected entry is clicked again.
    pub fn on_open(mut self, f: impl Fn(&mut S, &str) + Send + Sync + 'static) -> Self {
        self.on_open = Some(Arc::new(f));
        self
    }
    /// Start in grid view.
    pub fn grid(mut self) -> Self {
        self.grid = true;
        self
    }
}

impl<S: 'static> From<Files<S>> for Node<Act<S>> {
    fn from(fb: Files<S>) -> Self {
        type M<S> = Act<S>;
        let grid = local(fb.grid);
        let sel = local(None::<usize>);
        let cur = sel.get();
        let count = fb.entries.len();
        let view: Node<M<S>> = choose([("List", false), ("Grid", true)], grid.get())
            .local(&grid)
            .into();
        let crumbs: Node<M<S>> = super::nav::breadcrumb::<S>(fb.path.clone()).into();
        let head = row([
            crumbs,
            hfill(),
            caption(format!("{count} items")).dim().into(),
            view,
        ])
        .gap(14.0)
        .align_center()
        .fill_w();
        let open = fb.on_open.clone();
        let pick = move |i: usize, name: String| {
            let open = open.clone();
            move |s: &mut S| {
                if sel.get() == Some(i) {
                    if let Some(f) = &open {
                        f(s, &name);
                    }
                } else {
                    sel.set(Some(i));
                }
            }
        };
        let body: Node<M<S>> = if grid.get() {
            let cols = 4;
            col(fb.entries.chunks(cols).enumerate().map(|(r, chunk)| {
                row(chunk.iter().enumerate().map(|(c, e)| {
                    let i = r * cols + c;
                    let tile = col([
                        thumb::<M<S>>(&e.name, e.kind, 148.0, 96.0),
                        text(e.name.clone()).label().no_wrap().into(),
                        caption(if e.size.is_empty() {
                            e.modified.clone()
                        } else {
                            e.size.clone()
                        })
                        .dim()
                        .into(),
                    ])
                    .gap(4.0)
                    .pad(8.0);
                    Node::from(
                        tapped(tile, pick(i, e.name.clone()))
                            .selected(cur == Some(i))
                            .radius(10.0)
                            .into_element(),
                    )
                }))
                .gap(6.0)
            }))
            .gap(6.0)
            .into()
        } else {
            let hdr = row([
                crate::dsl::gap(26.0, 0.0),
                caption("Name").dim().w(10.0).into(),
                hfill(),
                caption("Size").dim().w(80.0).into(),
                caption("Modified").dim().w(110.0).into(),
            ])
            .gap(12.0)
            .pad_xy(10.0, 4.0)
            .fill_w();
            let mut rows: Vec<Node<M<S>>> = vec![hdr.into(), crate::dsl::rule()];
            for (i, e) in fb.entries.iter().enumerate() {
                let line = row([
                    nn(icon(glyph(e.kind))
                        .size(16.0)
                        .color(if e.kind == FileKind::Folder {
                            cx::palette().accent
                        } else {
                            cx::palette().text_secondary
                        })),
                    text(e.name.clone()).fill_w().into(),
                    caption(e.size.clone()).dim().w(80.0).into(),
                    caption(e.modified.clone()).dim().w(110.0).into(),
                ])
                .gap(12.0)
                .align_center()
                .pad_xy(10.0, 8.0)
                .fill_w();
                rows.push(
                    tapped(line, pick(i, e.name.clone()))
                        .selected(cur == Some(i))
                        .radius(6.0)
                        .into_element()
                        .into(),
                );
            }
            col(rows).gap(1.0).fill_w().into()
        };
        col([head.into(), body]).gap(12.0).fill_w().into()
    }
}

/// One tile of a [`gallery_grid`].
pub struct Tile {
    title: String,
    sub: String,
}

/// A tile: `tile("Harbour at dusk", "Mira · 2 MB")`.
pub fn tile(title: impl Into<String>, sub: impl Into<String>) -> Tile {
    Tile {
        title: title.into(),
        sub: sub.into(),
    }
}

/// A gallery grid under construction. See [`gallery_grid`].
pub struct Gallery<S> {
    tiles: Vec<Tile>,
    cols: usize,
    on_open: Option<Arc<dyn Fn(&mut S, usize) + Send + Sync>>,
}

/// Thumbnails in rows of `cols` (default 4) with a title and a line of detail, a selection ring
/// and an optional open handler: `gallery_grid([tile("Dusk", "2 MB")]).cols(3)`.
pub fn gallery_grid<S: 'static>(tiles: impl IntoIterator<Item = Tile>) -> Gallery<S> {
    Gallery {
        tiles: tiles.into_iter().collect(),
        cols: 4,
        on_open: None,
    }
}

impl<S: 'static> Gallery<S> {
    /// Tiles per row.
    pub fn cols(mut self, n: usize) -> Self {
        self.cols = n.max(1);
        self
    }
    /// Called with the index of a tile that is clicked while selected.
    pub fn on_open(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on_open = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Gallery<S>> for Node<Act<S>> {
    fn from(g: Gallery<S>) -> Self {
        let sel = local(None::<usize>);
        let cur = sel.get();
        let cols = g.cols;
        let open = g.on_open.clone();
        col(g.tiles.chunks(cols).enumerate().map(|(r, chunk)| {
            row(chunk.iter().enumerate().map(|(c, t)| {
                let i = r * cols + c;
                let open = open.clone();
                let tile = col([
                    thumb::<Act<S>>(&t.title, FileKind::Image, 168.0, 112.0),
                    text(t.title.clone()).label().no_wrap().into(),
                    caption(t.sub.clone()).dim().into(),
                ])
                .gap(4.0)
                .pad(8.0);
                Node::from(
                    tapped(tile, move |s| {
                        if sel.get() == Some(i) {
                            if let Some(f) = &open {
                                f(s, i);
                            }
                        } else {
                            sel.set(Some(i));
                        }
                    })
                    .selected(cur == Some(i))
                    .radius(10.0)
                    .into_element(),
                )
            }))
            .gap(8.0)
        }))
        .gap(8.0)
        .into()
    }
}

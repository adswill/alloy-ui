//! The layout inspector: an overlay that outlines every widget, names the one
//! under the cursor and shows its path from the root.
//!
//! Toggle it with F12 (debug builds, or any build run with `ALLOY_INSPECT=1`,
//! which `alloy dev` sets). It only reads the tree and layout the runtime
//! already has, so widgets need no code to be inspectable.

use crate::{
    draw::{Border, DrawCommand, DrawList, FontFamily},
    geometry::{Color, Point, Rectangle, Size},
    widget::{Element, Layout},
};

/// True when the inspector may be opened in this process.
pub fn available() -> bool {
    cfg!(debug_assertions) || std::env::var_os("ALLOY_INSPECT").is_some()
}

/// One widget in the flattened tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Distance from the root.
    pub depth: usize,
    /// Short widget type name, e.g. `Button`.
    pub name: String,
    /// Resolved bounds in logical pixels.
    pub bounds: Rectangle,
}

/// The widget type name without module path or generics.
pub fn short_name(full: &str) -> String {
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base).to_string()
}

/// Flatten `view` and `layout` into pre-order nodes.
pub fn nodes<M>(view: &Element<M>, layout: &Layout) -> Vec<Node> {
    fn walk<M>(e: &Element<M>, l: &Layout, depth: usize, out: &mut Vec<Node>) {
        out.push(Node {
            depth,
            name: short_name(e.0.debug_name()),
            bounds: l.bounds,
        });
        for (c, cl) in e.0.children().iter().zip(&l.children) {
            walk(c, cl, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(view, layout, 0, &mut out);
    out
}

/// The nodes containing `p`, root first, deepest last.
pub fn path_at(nodes: &[Node], p: Point) -> Vec<&Node> {
    let mut path: Vec<&Node> = Vec::new();
    for n in nodes {
        if n.bounds.contains(p) && n.depth == path.len() {
            path.push(n);
        }
    }
    path
}

const PALETTE: [u32; 6] = [0x3b82f6, 0x10b981, 0xf59e0b, 0xec4899, 0x8b5cf6, 0x06b6d4];

fn text(origin: Point, s: &str, color: Color) -> DrawCommand {
    DrawCommand::Text {
        origin,
        content: s.into(),
        size: 11.0,
        color,
        max_width: None,
        weight: 500,
        family: FontFamily::Mono,
        line_height: Some(14.0),
    }
}

fn chip(out: &mut DrawList, at: Point, s: &str, window: Size) {
    let w = s.chars().count() as f32 * 6.7 + 12.0;
    let x = at.x.clamp(4.0, (window.width - w - 4.0).max(4.0));
    let y = at.y.clamp(4.0, (window.height - 22.0).max(4.0));
    out.push(DrawCommand::Quad {
        bounds: Rectangle::new(Point { x, y }, Size::new(w, 20.0)),
        fill: Color::hex(0x111827).with_alpha(0.94),
        radius: 5.0,
        border: None,
        shadow: None,
    });
    out.push(text(
        Point {
            x: x + 6.0,
            y: y + 3.0,
        },
        s,
        Color::hex(0xf9fafb),
    ));
}

/// Append the overlay for a tree whose cursor is at `cursor`.
pub fn draw<M>(
    view: &Element<M>,
    layout: &Layout,
    cursor: Point,
    window: Size,
    out: &mut DrawList,
) {
    let all = nodes(view, layout);
    for n in &all {
        let c = Color::hex(PALETTE[n.depth % PALETTE.len()]);
        out.push(DrawCommand::Quad {
            bounds: n.bounds,
            fill: Color::TRANSPARENT,
            radius: 0.0,
            border: Some(Border {
                color: c.with_alpha(0.55),
                width: 1.0,
            }),
            shadow: None,
        });
    }
    let path = path_at(&all, cursor);
    let Some(hit) = path.last() else { return };
    let c = Color::hex(PALETTE[hit.depth % PALETTE.len()]);
    out.push(DrawCommand::Quad {
        bounds: hit.bounds,
        fill: c.with_alpha(0.18),
        radius: 0.0,
        border: Some(Border {
            color: c,
            width: 2.0,
        }),
        shadow: None,
    });
    let b = hit.bounds;
    let label = format!(
        "{}  {:.0}x{:.0} @ {:.0},{:.0}",
        hit.name, b.size.width, b.size.height, b.origin.x, b.origin.y
    );
    let below = b.origin.y + b.size.height + 6.0;
    let y = if below + 22.0 > window.height {
        b.origin.y - 26.0
    } else {
        below
    };
    chip(out, Point { x: b.origin.x, y }, &label, window);
    let trail = path
        .iter()
        .map(|n| n.name.as_str())
        .collect::<Vec<_>>()
        .join(" > ");
    chip(
        out,
        Point {
            x: 4.0,
            y: window.height - 24.0,
        },
        &trail,
        window,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_drop_path_and_generics() {
        assert_eq!(
            short_name("alloy_ui_widgets::button::Button<alloy::Act<S>>"),
            "Button"
        );
        assert_eq!(short_name("Plain"), "Plain");
    }

    #[test]
    fn path_follows_containment_by_depth() {
        let r = |x, w| Rectangle::new(Point { x, y: 0.0 }, Size::new(w, 10.0));
        let n = |depth, b| Node {
            depth,
            name: format!("n{depth}"),
            bounds: b,
        };
        let all = vec![
            n(0, r(0.0, 100.0)),
            n(1, r(0.0, 40.0)),
            n(1, r(50.0, 40.0)),
            n(2, r(50.0, 10.0)),
        ];
        let at = |x| {
            path_at(&all, Point { x, y: 5.0 })
                .iter()
                .map(|n| n.depth)
                .collect::<Vec<_>>()
        };
        assert_eq!(at(55.0), vec![0, 1, 2]);
        assert_eq!(at(10.0), vec![0, 1]);
        assert_eq!(at(95.0), vec![0]);
    }
}

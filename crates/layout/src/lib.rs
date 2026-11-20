//! Alloy's own flexbox-style layout engine (row/column, padding, spacing,
//! alignment; fill/shrink/fixed sizing).
//!
//! Build a [`Node`] tree and call [`compute`] to get absolute bounds as an
//! [`alloy_ui_core::Layout`] tree with the same shape as the input.
#![warn(missing_docs)]

pub use alloy_ui_core::style::*;
use alloy_ui_core::{Layout, Point, Rectangle, Size};

/// A layout input: a style plus either a leaf intrinsic size or children.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// How the node is sized and how it arranges children.
    pub style: Style,
    /// Intrinsic content size of a leaf (ignored when `children` is non-empty).
    pub intrinsic: Size,
    /// Child nodes; empty for a leaf.
    pub children: Vec<Node>,
}

impl Node {
    /// A leaf with the given intrinsic (content) size.
    pub fn leaf(width: f32, height: f32) -> Self {
        Self {
            style: Style::default(),
            intrinsic: Size::new(width, height),
            children: Vec::new(),
        }
    }

    /// A container flowing `children` along `axis`.
    pub fn container(axis: Axis, children: Vec<Node>) -> Self {
        Self {
            style: Style {
                axis,
                ..Style::default()
            },
            intrinsic: Size::default(),
            children,
        }
    }

    /// A horizontal container.
    pub fn row(children: Vec<Node>) -> Self {
        Self::container(Axis::Horizontal, children)
    }

    /// A vertical container.
    pub fn column(children: Vec<Node>) -> Self {
        Self::container(Axis::Vertical, children)
    }

    /// Set the width sizing.
    pub fn width(mut self, l: Length) -> Self {
        self.style.width = l;
        self
    }

    /// Set the height sizing.
    pub fn height(mut self, l: Length) -> Self {
        self.style.height = l;
        self
    }

    /// Set padding.
    pub fn padding(mut self, p: Padding) -> Self {
        self.style.padding = p;
        self
    }

    /// Set main-axis spacing between children.
    pub fn spacing(mut self, s: f32) -> Self {
        self.style.spacing = s;
        self
    }

    /// Set the cross-axis alignment of children.
    pub fn align(mut self, a: Align) -> Self {
        self.style.align = a;
        self
    }

    /// Override the parent's cross-axis alignment for this node.
    pub fn align_self(mut self, a: Align) -> Self {
        self.style.align_self = Some(a);
        self
    }

    /// Set min/max limits.
    pub fn limits(mut self, l: Limits) -> Self {
        self.style.limits = l;
        self
    }

    fn length(&self, axis: Axis) -> Length {
        match axis {
            Axis::Horizontal => self.style.width,
            Axis::Vertical => self.style.height,
        }
    }

    /// Size of the node's content box (children plus spacing, or the leaf's
    /// intrinsic size), before padding, explicit lengths and limits.
    fn content_size(&self) -> Size {
        if self.children.is_empty() {
            return Size::new(clean(self.intrinsic.width), clean(self.intrinsic.height));
        }
        let axis = self.style.axis;
        let gaps = clean(self.style.spacing) * (self.children.len() - 1) as f32;
        let (mut main, mut cross) = (gaps, 0.0f32);
        for c in &self.children {
            let s = c.preferred_size();
            main += main_of(s, axis);
            cross = cross.max(cross_of(s, axis));
        }
        make_size(axis, main, cross)
    }

    /// The size this node asks for when not given a definite size: `Fixed`
    /// lengths as stated, everything else as its content plus padding, then
    /// clamped to the limits.
    pub fn preferred_size(&self) -> Size {
        let content = self.content_size();
        let p = &self.style.padding;
        let natural = |len: Length, c: f32, pad: f32| match len {
            Length::Fixed(v) => clean(v),
            _ => c + pad,
        };
        let size = Size::new(
            natural(self.style.width, content.width, p.horizontal()),
            natural(self.style.height, content.height, p.vertical()),
        );
        self.style.limits.clamp(size)
    }
}

fn main_of(s: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => s.width,
        Axis::Vertical => s.height,
    }
}

fn cross_of(s: Size, axis: Axis) -> f32 {
    main_of(s, flip(axis))
}

fn flip(axis: Axis) -> Axis {
    match axis {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    }
}

fn make_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

/// Lay out `root` inside `available`, with its top-left corner at `origin`.
///
/// The root resolves each axis by its [`Length`]: `Fill` takes the available
/// space, `Shrink` its preferred size, `Fixed` the stated value; the result is
/// clamped to the node's limits. Children that do not fit overflow (they are
/// never shrunk below their own minimum and never cause a panic or NaN).
/// The returned [`Layout`] has exactly the same shape as the input tree.
///
/// Cost is O(n * depth) because preferred sizes are recomputed per level.
pub fn compute(root: &Node, origin: Point, available: Size) -> Layout {
    let pref = root.preferred_size();
    let pick = |len: Length, avail: f32, pref: f32| match len {
        Length::Fill(w) if w > 0 => clean(avail),
        _ => pref,
    };
    let size = root.style.limits.clamp(Size::new(
        pick(root.style.width, available.width, pref.width),
        pick(root.style.height, available.height, pref.height),
    ));
    place(root, origin, size)
}

/// Place `node` (whose final border-box `size` is already decided).
fn place(node: &Node, origin: Point, size: Size) -> Layout {
    let bounds = Rectangle::new(origin, size);
    if node.children.is_empty() {
        return Layout {
            bounds,
            children: Vec::new(),
        };
    }
    let style = &node.style;
    let axis = style.axis;
    let pad = &style.padding;
    let inner_w = (size.width - pad.horizontal()).max(0.0);
    let inner_h = (size.height - pad.vertical()).max(0.0);
    let inner_main = main_of(Size::new(inner_w, inner_h), axis);
    let inner_cross = cross_of(Size::new(inner_w, inner_h), axis);
    let spacing = clean(style.spacing);
    let n = node.children.len();

    // Main-axis sizes.
    let mut mains: Vec<f32> = node
        .children
        .iter()
        .map(|c| main_of(c.preferred_size(), axis))
        .collect();
    let flex: Vec<Option<(u16, f32, f32)>> = node
        .children
        .iter()
        .map(|c| match c.length(axis) {
            Length::Fill(w) if w > 0 => {
                let (lo, hi) = ordered(
                    main_of(c.style.limits.min, axis),
                    main_of(c.style.limits.max, axis),
                );
                Some((w, lo, hi))
            }
            _ => None,
        })
        .collect();
    let fixed_total: f32 = mains
        .iter()
        .zip(&flex)
        .filter(|(_, f)| f.is_none())
        .map(|(m, _)| *m)
        .sum();
    let free = (inner_main - spacing * (n - 1) as f32 - fixed_total).max(0.0);
    distribute(free, &flex, &mut mains);

    // Cross-axis sizes and placement.
    let content_origin = Point::new(origin.x + clean(pad.left), origin.y + clean(pad.top));
    let mut cursor = 0.0f32;
    let mut children = Vec::with_capacity(n);
    for (c, &m) in node.children.iter().zip(&mains) {
        let pref = c.preferred_size();
        let cross = match c.length(flip(axis)) {
            Length::Fill(w) if w > 0 => {
                let (lo, hi) = ordered(
                    cross_of(c.style.limits.min, axis),
                    cross_of(c.style.limits.max, axis),
                );
                inner_cross.clamp(lo, hi)
            }
            _ => cross_of(pref, axis),
        };
        let offset = match c.style.align_self.unwrap_or(style.align) {
            Align::Start => 0.0,
            Align::Center => ((inner_cross - cross) / 2.0).max(0.0),
            Align::End => (inner_cross - cross).max(0.0),
        };
        let pos = match axis {
            Axis::Horizontal => Point::new(content_origin.x + cursor, content_origin.y + offset),
            Axis::Vertical => Point::new(content_origin.x + offset, content_origin.y + cursor),
        };
        children.push(place(c, pos, make_size(axis, m, cross)));
        cursor += m + spacing;
    }
    Layout { bounds, children }
}

/// Share `free` among flexible items by weight, honouring each item's
/// `(weight, min, max)`; writes results into `mains`. Violating items are
/// frozen at their limit and the remainder is redistributed (CSS flexbox).
fn distribute(free: f32, flex: &[Option<(u16, f32, f32)>], mains: &mut [f32]) {
    let mut active: Vec<usize> = (0..flex.len()).filter(|&i| flex[i].is_some()).collect();
    let mut free = free;
    while !active.is_empty() {
        let total_w: f32 = active.iter().map(|&i| f32::from(flex[i].unwrap().0)).sum();
        let raw: Vec<f32> = active
            .iter()
            .map(|&i| free * f32::from(flex[i].unwrap().0) / total_w)
            .collect();
        let clamped: Vec<f32> = active
            .iter()
            .zip(&raw)
            .map(|(&i, &r)| {
                let (_, lo, hi) = flex[i].unwrap();
                r.clamp(lo, hi)
            })
            .collect();
        let violation: f32 = clamped.iter().zip(&raw).map(|(c, r)| c - r).sum();
        let freeze = |k: usize| {
            if violation == 0.0 {
                true
            } else if violation > 0.0 {
                clamped[k] > raw[k]
            } else {
                clamped[k] < raw[k]
            }
        };
        let mut next = Vec::new();
        for (k, &i) in active.iter().enumerate() {
            if freeze(k) {
                mains[i] = clamped[k];
                free -= clamped[k];
            } else {
                next.push(i);
            }
        }
        free = free.max(0.0);
        if next.len() == active.len() {
            // Cannot happen (a violation implies a frozen item); guard anyway.
            for (k, &i) in active.iter().enumerate() {
                mains[i] = clamped[k];
            }
            break;
        }
        active = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: Point = Point::new(0.0, 0.0);

    fn sz(w: f32, h: f32) -> Size {
        Size::new(w, h)
    }
    fn b(l: &Layout) -> (f32, f32, f32, f32) {
        let r = l.bounds;
        (r.origin.x, r.origin.y, r.size.width, r.size.height)
    }
    fn fill() -> Length {
        Length::Fill(1)
    }
    fn finite(l: &Layout) -> bool {
        let r = l.bounds;
        [r.origin.x, r.origin.y, r.size.width, r.size.height]
            .iter()
            .all(|v| v.is_finite())
            && l.children.iter().all(finite)
    }

    #[test]
    fn leaf_shrink_uses_intrinsic() {
        let l = compute(&Node::leaf(30.0, 10.0), O, sz(100.0, 100.0));
        assert_eq!(b(&l), (0.0, 0.0, 30.0, 10.0));
    }

    #[test]
    fn leaf_fixed_overrides_intrinsic() {
        let n = Node::leaf(30.0, 10.0)
            .width(Length::Fixed(50.0))
            .height(Length::Fixed(5.0));
        assert_eq!(b(&compute(&n, O, sz(100.0, 100.0))), (0.0, 0.0, 50.0, 5.0));
    }

    #[test]
    fn root_fill_takes_available() {
        let n = Node::leaf(1.0, 1.0).width(fill()).height(fill());
        assert_eq!(b(&compute(&n, O, sz(80.0, 60.0))), (0.0, 0.0, 80.0, 60.0));
    }

    #[test]
    fn root_origin_is_respected() {
        let n = Node::row(vec![Node::leaf(10.0, 10.0)]).padding(Padding::all(2.0));
        let l = compute(&n, Point::new(5.0, 7.0), sz(100.0, 100.0));
        assert_eq!(b(&l.children[0]), (7.0, 9.0, 10.0, 10.0));
    }

    #[test]
    fn empty_container_shrinks_to_padding() {
        let n = Node::row(vec![]).padding(Padding::symmetric(3.0, 4.0));
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(b(&l), (0.0, 0.0, 6.0, 8.0));
        assert!(l.children.is_empty());
    }

    #[test]
    fn empty_container_fill() {
        let n = Node::column(vec![]).width(fill()).height(fill());
        assert_eq!(b(&compute(&n, O, sz(9.0, 8.0))), (0.0, 0.0, 9.0, 8.0));
    }

    #[test]
    fn row_stacks_horizontally_with_spacing() {
        let n = Node::row(vec![Node::leaf(10.0, 5.0), Node::leaf(20.0, 8.0)]).spacing(4.0);
        let l = compute(&n, O, sz(200.0, 200.0));
        assert_eq!(b(&l), (0.0, 0.0, 34.0, 8.0));
        assert_eq!(b(&l.children[0]), (0.0, 0.0, 10.0, 5.0));
        assert_eq!(b(&l.children[1]), (14.0, 0.0, 20.0, 8.0));
    }

    #[test]
    fn column_stacks_vertically() {
        let n = Node::column(vec![Node::leaf(10.0, 5.0), Node::leaf(20.0, 8.0)]).spacing(1.0);
        let l = compute(&n, O, sz(200.0, 200.0));
        assert_eq!(b(&l), (0.0, 0.0, 20.0, 14.0));
        assert_eq!(b(&l.children[1]), (0.0, 6.0, 20.0, 8.0));
    }

    #[test]
    fn padding_offsets_children_and_grows_container() {
        let n = Node::row(vec![Node::leaf(10.0, 10.0)]).padding(Padding {
            top: 1.0,
            right: 2.0,
            bottom: 3.0,
            left: 4.0,
        });
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(b(&l), (0.0, 0.0, 16.0, 14.0));
        assert_eq!(b(&l.children[0]), (4.0, 1.0, 10.0, 10.0));
    }

    #[test]
    fn fill_splits_equally() {
        let n = Node::row(vec![
            Node::leaf(0.0, 10.0).width(fill()),
            Node::leaf(0.0, 10.0).width(fill()),
        ])
        .width(fill());
        let l = compute(&n, O, sz(100.0, 50.0));
        assert_eq!(b(&l.children[0]), (0.0, 0.0, 50.0, 10.0));
        assert_eq!(b(&l.children[1]), (50.0, 0.0, 50.0, 10.0));
    }

    #[test]
    fn fill_splits_by_weight() {
        let n = Node::row(vec![
            Node::leaf(0.0, 1.0).width(Length::Fill(1)),
            Node::leaf(0.0, 1.0).width(Length::Fill(3)),
        ])
        .width(fill());
        let l = compute(&n, O, sz(100.0, 50.0));
        assert_eq!(l.children[0].bounds.size.width, 25.0);
        assert_eq!(l.children[1].bounds.size.width, 75.0);
        assert_eq!(l.children[1].bounds.origin.x, 25.0);
    }

    #[test]
    fn fill_gets_leftover_after_fixed_shrink_spacing_padding() {
        let n = Node::row(vec![
            Node::leaf(20.0, 1.0),
            Node::leaf(0.0, 1.0).width(fill()),
            Node::leaf(0.0, 1.0).width(Length::Fixed(30.0)),
        ])
        .spacing(5.0)
        .padding(Padding::all(5.0))
        .width(fill());
        let l = compute(&n, O, sz(200.0, 50.0));
        // 200 - 10 pad - 10 gaps - 20 - 30 = 130
        assert_eq!(l.children[1].bounds.size.width, 130.0);
        assert_eq!(
            l.children[2].bounds.origin.x,
            5.0 + 20.0 + 5.0 + 130.0 + 5.0
        );
    }

    #[test]
    fn fill_in_vertical_axis() {
        let n = Node::column(vec![
            Node::leaf(5.0, 10.0),
            Node::leaf(5.0, 0.0).height(Length::Fill(2)),
            Node::leaf(5.0, 0.0).height(Length::Fill(1)),
        ])
        .height(fill());
        let l = compute(&n, O, sz(10.0, 100.0));
        assert_eq!(l.children[1].bounds.size.height, 60.0);
        assert_eq!(l.children[2].bounds.size.height, 30.0);
        assert_eq!(l.children[2].bounds.origin.y, 70.0);
    }

    #[test]
    fn fill_in_shrink_container_uses_content_size() {
        let n = Node::row(vec![Node::leaf(12.0, 3.0).width(fill())]);
        let l = compute(&n, O, sz(500.0, 500.0));
        assert_eq!(b(&l), (0.0, 0.0, 12.0, 3.0));
        assert_eq!(l.children[0].bounds.size.width, 12.0);
    }

    #[test]
    fn fill_zero_weight_acts_like_shrink() {
        let n = Node::row(vec![
            Node::leaf(10.0, 1.0).width(Length::Fill(0)),
            Node::leaf(0.0, 1.0).width(fill()),
        ])
        .width(fill());
        let l = compute(&n, O, sz(100.0, 10.0));
        assert_eq!(l.children[0].bounds.size.width, 10.0);
        assert_eq!(l.children[1].bounds.size.width, 90.0);
    }

    #[test]
    fn cross_fill_stretches() {
        let n = Node::row(vec![Node::leaf(10.0, 1.0).height(fill())])
            .height(Length::Fixed(40.0))
            .padding(Padding::all(5.0));
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(l.children[0].bounds.size.height, 30.0);
        assert_eq!(l.children[0].bounds.origin.y, 5.0);
    }

    #[test]
    fn align_center_and_end() {
        let mk = |a| {
            Node::row(vec![Node::leaf(10.0, 10.0)])
                .height(Length::Fixed(50.0))
                .align(a)
        };
        let c = compute(&mk(Align::Center), O, sz(100.0, 100.0));
        let e = compute(&mk(Align::End), O, sz(100.0, 100.0));
        let s = compute(&mk(Align::Start), O, sz(100.0, 100.0));
        assert_eq!(c.children[0].bounds.origin.y, 20.0);
        assert_eq!(e.children[0].bounds.origin.y, 40.0);
        assert_eq!(s.children[0].bounds.origin.y, 0.0);
    }

    #[test]
    fn align_in_column_uses_x() {
        let n = Node::column(vec![Node::leaf(10.0, 10.0)])
            .width(Length::Fixed(50.0))
            .align(Align::End);
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(l.children[0].bounds.origin.x, 40.0);
    }

    #[test]
    fn align_self_overrides_parent() {
        let n = Node::row(vec![
            Node::leaf(10.0, 10.0),
            Node::leaf(10.0, 10.0).align_self(Align::End),
        ])
        .height(Length::Fixed(30.0))
        .align(Align::Start);
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(l.children[0].bounds.origin.y, 0.0);
        assert_eq!(l.children[1].bounds.origin.y, 20.0);
    }

    #[test]
    fn shrink_container_sizes_to_largest_cross() {
        let n = Node::row(vec![Node::leaf(1.0, 5.0), Node::leaf(1.0, 9.0)]);
        assert_eq!(compute(&n, O, sz(100.0, 100.0)).bounds.size.height, 9.0);
    }

    #[test]
    fn nested_containers() {
        let inner = Node::column(vec![Node::leaf(10.0, 10.0), Node::leaf(10.0, 10.0)])
            .spacing(2.0)
            .padding(Padding::all(1.0));
        let n = Node::row(vec![Node::leaf(5.0, 5.0), inner]).spacing(3.0);
        let l = compute(&n, O, sz(200.0, 200.0));
        assert_eq!(b(&l.children[1]), (8.0, 0.0, 12.0, 24.0));
        assert_eq!(b(&l.children[1].children[1]), (9.0, 13.0, 10.0, 10.0));
        assert_eq!(l.bounds.size, sz(20.0, 24.0));
    }

    #[test]
    fn nested_fill_resolves_through_levels() {
        let inner = Node::row(vec![
            Node::leaf(0.0, 10.0).width(fill()),
            Node::leaf(0.0, 10.0).width(fill()),
        ])
        .width(fill());
        let n = Node::column(vec![inner])
            .width(fill())
            .padding(Padding::all(10.0));
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(b(&l.children[0]), (10.0, 10.0, 80.0, 10.0));
        assert_eq!(l.children[0].children[1].bounds.origin.x, 50.0);
    }

    #[test]
    fn fixed_container_holds_fill_children() {
        let n = Node::row(vec![Node::leaf(0.0, 1.0).width(fill())]).width(Length::Fixed(64.0));
        let l = compute(&n, O, sz(10.0, 10.0));
        assert_eq!(l.bounds.size.width, 64.0);
        assert_eq!(l.children[0].bounds.size.width, 64.0);
    }

    #[test]
    fn min_limit_raises_shrink() {
        let n = Node::leaf(5.0, 5.0).limits(Limits::new(sz(20.0, 0.0), Limits::NONE.max));
        assert_eq!(compute(&n, O, sz(100.0, 100.0)).bounds.size, sz(20.0, 5.0));
    }

    #[test]
    fn max_limit_caps_fixed_and_fill() {
        let n = Node::leaf(5.0, 5.0)
            .width(fill())
            .height(Length::Fixed(90.0))
            .limits(Limits::new(sz(0.0, 0.0), sz(40.0, 30.0)));
        assert_eq!(compute(&n, O, sz(100.0, 100.0)).bounds.size, sz(40.0, 30.0));
    }

    #[test]
    fn fill_child_max_redistributes_surplus() {
        let capped = Node::leaf(0.0, 1.0)
            .width(fill())
            .limits(Limits::new(sz(0.0, 0.0), sz(10.0, f32::INFINITY)));
        let n = Node::row(vec![capped, Node::leaf(0.0, 1.0).width(fill())]).width(fill());
        let l = compute(&n, O, sz(100.0, 10.0));
        assert_eq!(l.children[0].bounds.size.width, 10.0);
        assert_eq!(l.children[1].bounds.size.width, 90.0);
        assert_eq!(l.children[1].bounds.origin.x, 10.0);
    }

    #[test]
    fn fill_child_min_takes_from_others() {
        let big = Node::leaf(0.0, 1.0)
            .width(fill())
            .limits(Limits::new(sz(70.0, 0.0), Limits::NONE.max));
        let n = Node::row(vec![big, Node::leaf(0.0, 1.0).width(fill())]).width(fill());
        let l = compute(&n, O, sz(100.0, 10.0));
        assert_eq!(l.children[0].bounds.size.width, 70.0);
        assert_eq!(l.children[1].bounds.size.width, 30.0);
    }

    #[test]
    fn overflow_fixed_children_do_not_panic() {
        let n = Node::row(vec![
            Node::leaf(80.0, 10.0),
            Node::leaf(80.0, 10.0),
            Node::leaf(0.0, 10.0).width(fill()),
        ])
        .width(Length::Fixed(100.0));
        let l = compute(&n, O, sz(100.0, 100.0));
        assert_eq!(l.children[2].bounds.size.width, 0.0);
        assert_eq!(l.children[1].bounds.origin.x, 80.0);
        assert!(finite(&l));
    }

    #[test]
    fn padding_larger_than_box_is_safe() {
        let n = Node::row(vec![Node::leaf(0.0, 0.0).width(fill()).height(fill())])
            .width(Length::Fixed(10.0))
            .height(Length::Fixed(10.0))
            .padding(Padding::all(50.0));
        let l = compute(&n, O, sz(10.0, 10.0));
        assert_eq!(l.bounds.size, sz(10.0, 10.0));
        assert_eq!(l.children[0].bounds.size, sz(0.0, 0.0));
        assert!(finite(&l));
    }

    #[test]
    fn nan_inf_negative_inputs_are_sanitised() {
        let n = Node::row(vec![
            Node::leaf(f32::NAN, -5.0),
            Node::leaf(f32::INFINITY, 1.0).width(Length::Fixed(f32::NAN)),
            Node::leaf(1.0, 1.0).width(fill()),
        ])
        .spacing(f32::NAN)
        .padding(Padding::all(-3.0))
        .width(fill());
        let l = compute(&n, O, sz(f32::NAN, f32::NAN));
        assert!(finite(&l));
        assert_eq!(l.bounds.size.width, 0.0);
    }

    #[test]
    fn zero_available_space() {
        let n = Node::row(vec![Node::leaf(0.0, 0.0).width(fill())])
            .width(fill())
            .height(fill());
        let l = compute(&n, O, sz(0.0, 0.0));
        assert_eq!(b(&l), (0.0, 0.0, 0.0, 0.0));
        assert!(finite(&l));
    }

    #[test]
    fn inverted_limits_prefer_min() {
        let lim = Limits::new(sz(50.0, 0.0), sz(10.0, 10.0));
        assert_eq!(lim.clamp(sz(1.0, 100.0)), sz(50.0, 10.0));
    }

    #[test]
    fn limits_clamp_sanitises() {
        assert_eq!(Limits::NONE.clamp(sz(f32::NAN, -1.0)), sz(0.0, 0.0));
        let lim = Limits::new(sz(0.0, 0.0), sz(f32::NAN, 5.0));
        assert_eq!(lim.clamp(sz(1e9, 9.0)), sz(1e9, 5.0));
        assert_eq!(Limits::default(), Limits::NONE);
        assert_eq!(
            Limits::exact(sz(3.0, 4.0)).clamp(sz(9.0, 9.0)),
            sz(3.0, 4.0)
        );
    }

    #[test]
    fn padding_helpers() {
        assert_eq!(Padding::all(2.0).horizontal(), 4.0);
        assert_eq!(Padding::symmetric(1.0, 3.0).vertical(), 6.0);
        assert_eq!(Padding::ZERO, Padding::default());
    }

    #[test]
    fn output_shape_matches_input() {
        let n = Node::row(vec![
            Node::column(vec![Node::leaf(1.0, 1.0), Node::row(vec![])]),
            Node::leaf(2.0, 2.0),
        ]);
        let l = compute(&n, O, sz(50.0, 50.0));
        assert_eq!(l.children.len(), 2);
        assert_eq!(l.children[0].children.len(), 2);
        assert!(l.children[0].children[1].children.is_empty());
    }

    #[test]
    fn many_weighted_children_sum_to_available() {
        let kids: Vec<Node> = (1..=7)
            .map(|w| Node::leaf(0.0, 1.0).width(Length::Fill(w)))
            .collect();
        let n = Node::row(kids).width(fill());
        let l = compute(&n, O, sz(280.0, 10.0));
        let sum: f32 = l.children.iter().map(|c| c.bounds.size.width).sum();
        assert!((sum - 280.0).abs() < 1e-3);
    }

    #[test]
    fn spacing_only_between_children() {
        let n = Node::row(vec![Node::leaf(0.0, 0.0), Node::leaf(0.0, 0.0)]).spacing(7.0);
        assert_eq!(compute(&n, O, sz(100.0, 100.0)).bounds.size.width, 7.0);
        let one = Node::row(vec![Node::leaf(3.0, 0.0)]).spacing(7.0);
        assert_eq!(compute(&one, O, sz(100.0, 100.0)).bounds.size.width, 3.0);
    }
}

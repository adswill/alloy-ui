//! Avatars, avatar groups and presence.

use super::*;
use alloy_ui_core::{Color, Point, Rectangle, Size};
use alloy_ui_layout::Length;
use alloy_ui_widgets::Frame;

/// Whether someone is around.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Presence {
    /// Active now.
    Online,
    /// Idle.
    Away,
    /// Do not disturb.
    Busy,
    /// Not here.
    Offline,
    /// Playing a game right now (Steam-style green).
    InGame,
}

impl Presence {
    fn color(self, p: &alloy_ui_theme::Palette) -> Color {
        match self {
            Presence::Online => p.success,
            Presence::Away => p.warning,
            Presence::Busy => p.danger,
            Presence::Offline => p.text_disabled,
            Presence::InGame => Color::hex(0x90BA3C),
        }
    }
}

const HUES: [u32; 6] = [0x2F6FEB, 0x0E8F7E, 0xC2571A, 0x8B5CF6, 0xC9356B, 0x6B8E23];

fn initials(name: &str) -> String {
    let mut it = name.split_whitespace();
    let a = it.next().and_then(|w| w.chars().next());
    let b = it.last().and_then(|w| w.chars().next());
    a.into_iter()
        .chain(b)
        .flat_map(|c| c.to_uppercase())
        .collect()
}

fn face(f: &mut Frame, c: Point, r: f32, name: &str, ring: Option<Color>) {
    let p = f.palette();
    if let Some(ring) = ring {
        f.circle(c, r + 2.0, ring);
    }
    let hue = Color::hex(HUES[hash(name) as usize % HUES.len()]);
    f.circle(c, r, mix(p.surface, hue, 0.2));
    f.stroke_circle(c, r, 1.0, mix(p.surface, hue, 0.35));
    let ink = mix(p.text, hue, 0.55);
    let s = initials(name);
    let size = (r * 0.74).max(9.0);
    let w = f.text_width(&s, size) * 1.06;
    f.text_weight(pt(c.x - w / 2.0, c.y - size * 0.62), s, size, 600, ink);
}

fn square_face(f: &mut Frame, s: f32, name: &str, presence: Option<Presence>) {
    let p = f.palette();
    let hue = Color::hex(HUES[hash(name) as usize % HUES.len()]);
    let r = (s * 0.1).max(2.0);
    let all = Rectangle::new(pt(0.0, 0.0), Size::new(s, s));
    f.round_rect(all, r, mix(p.surface, hue, 0.2));
    let text = initials(name);
    let size = (s * 0.37).max(9.0);
    let w = f.text_width(&text, size) * 1.06;
    f.text_weight(
        pt((s - w) / 2.0, s / 2.0 - size * 0.62),
        text,
        size,
        600,
        mix(p.text, hue, 0.55),
    );
    match presence {
        Some(pr) => {
            // Stroke is centred on the rect, so inset by half the width to stay inside.
            let inner = Rectangle::new(pt(1.0, 1.0), Size::new(s - 2.0, s - 2.0));
            f.stroke_rect(inner, (r - 1.0).max(1.0), 2.0, pr.color(&p));
        }
        None => f.stroke_rect(all, r, 1.0, mix(p.surface, hue, 0.35)),
    }
}

fn dot(f: &mut Frame, c: Point, r: f32, pr: Presence) {
    let p = f.palette();
    f.circle(c, r + 2.0, p.surface);
    f.circle(c, r, pr.color(&p));
}

/// An avatar under construction. See [`avatar`].
pub struct Avatar {
    name: String,
    size: f32,
    presence: Option<Presence>,
    square: bool,
}

/// A round avatar with the person's initials on a tint picked from their name:
/// `avatar("Mira Okafor").presence(Presence::Online)`.
pub fn avatar(name: impl Into<String>) -> Avatar {
    Avatar {
        name: name.into(),
        size: 32.0,
        presence: None,
        square: false,
    }
}

impl Avatar {
    /// Diameter in pixels (default 32).
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// A square avatar (slightly rounded corners). With a presence set it gets a 2px border in the
    /// status colour instead of a dot, the way Steam friend lists do:
    /// `avatar("Mira").square().presence(Presence::InGame)`.
    pub fn square(mut self) -> Self {
        self.square = true;
        self
    }
    /// A status dot at the bottom right.
    pub fn presence(mut self, p: Presence) -> Self {
        self.presence = Some(p);
        self
    }
}

impl<M: 'static> From<Avatar> for Node<M> {
    fn from(a: Avatar) -> Self {
        let s = a.size;
        paint(Length::Fixed(s), s, move |f| {
            if a.square {
                square_face(f, s, &a.name, a.presence);
                return;
            }
            let c = pt(s / 2.0, s / 2.0);
            face(f, c, s / 2.0, &a.name, None);
            if let Some(pr) = a.presence {
                let r = (s * 0.14).max(3.5);
                dot(f, pt(s - r - 1.0, s - r - 1.0), r, pr);
            }
        })
    }
}

/// An overlapping stack of avatars. See [`avatar_group`].
pub struct AvatarGroup {
    names: Vec<String>,
    max: usize,
    size: f32,
}

/// Overlapping avatars with a "+N" chip for the rest: `avatar_group(["Mira", "Jon", "Sam"]).max(3)`.
pub fn avatar_group<S: Into<String>>(names: impl IntoIterator<Item = S>) -> AvatarGroup {
    AvatarGroup {
        names: names.into_iter().map(Into::into).collect(),
        max: 4,
        size: 30.0,
    }
}

impl AvatarGroup {
    /// How many faces to show before the "+N" chip (default 4).
    pub fn max(mut self, n: usize) -> Self {
        self.max = n.max(1);
        self
    }
    /// Diameter in pixels (default 30).
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
}

impl<M: 'static> From<AvatarGroup> for Node<M> {
    fn from(g: AvatarGroup) -> Self {
        let s = g.size;
        let step = s * 0.72;
        let shown = g.names.len().min(g.max);
        let extra = g.names.len() - shown;
        let slots = shown + usize::from(extra > 0);
        let width = s + step * (slots.max(1) - 1) as f32 + 4.0;
        paint(Length::Fixed(width), s + 4.0, move |f| {
            let p = f.palette();
            let y = s / 2.0 + 2.0;
            for (i, n) in g.names.iter().take(shown).enumerate() {
                face(
                    f,
                    pt(s / 2.0 + 2.0 + i as f32 * step, y),
                    s / 2.0 - 2.0,
                    n,
                    Some(p.surface),
                );
            }
            if extra > 0 {
                let c = pt(s / 2.0 + 2.0 + shown as f32 * step, y);
                f.circle(c, s / 2.0, p.surface);
                f.circle(c, s / 2.0 - 2.0, p.border);
                let t = format!("+{extra}");
                let size = (s * 0.38).max(10.0);
                let w = f.text_width(&t, size) * 1.06;
                f.text_weight(
                    pt(c.x - w / 2.0, c.y - size * 0.62),
                    t,
                    size,
                    600,
                    p.text_secondary,
                );
            }
        })
    }
}

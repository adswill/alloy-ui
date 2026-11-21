//! Audio and video playback surface: transport controls around a pluggable
//! [`MediaSource`].
//!
//! This crate ships no decoder: codecs are large, patent-encumbered and OS
//! specific, which cuts against "tiny binaries". The optional `alloy-ui-media` crate adds
//! real ones behind feature flags. What this crate ships is the part every
//! player needs and nobody wants to rewrite: the picture area, a seek bar with
//! scrubbing, play/pause, volume, keyboard control and a clock that drives
//! redraws only while playing. A decoder crate (ffmpeg, gstreamer, AVFoundation,
//! Media Foundation) implements [`MediaSource`] and paints its current picture
//! into the [`Frame`]; the demo [`TestCard`] shows the contract without one.

use crate::canvas::Frame;
use crate::util::{self, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// What a player needs from a decoder. Implement this over your decoding
/// library; the player handles everything around the picture.
pub trait MediaSource {
    /// Total length in milliseconds.
    fn duration_ms(&self) -> u64;
    /// Picture width divided by height. Audio-only sources return `None` and
    /// get a visualiser area instead.
    fn aspect(&self) -> Option<f32>;
    /// Paint the picture for time `t_ms` over the whole of `frame`. A decoder
    /// registers its current frame as a renderer image and calls
    /// [`Frame::image`]; a procedural source just draws.
    fn paint(&self, frame: &mut Frame, t_ms: u64);
    /// Called every time the player is drawn, before [`MediaSource::paint`], with the state the
    /// UI wants: the position `t_ms`, whether it should be `playing`, and the output `volume`
    /// (0.0 when muted). A source with real sound reconciles itself here: start or stop the
    /// audio, jump when `t_ms` has drifted from its own clock, set the gain. The default does
    /// nothing, which suits a procedural source.
    fn sync(&self, _t_ms: u64, _playing: bool, _volume: f32) {}
}

/// A shared source is a source, so a decoder can live outside the view (in your state) while the
/// view hands the player a clone of its `Rc` on every rebuild.
impl<T: MediaSource + ?Sized> MediaSource for Rc<T> {
    fn duration_ms(&self) -> u64 {
        (**self).duration_ms()
    }
    fn aspect(&self) -> Option<f32> {
        (**self).aspect()
    }
    fn paint(&self, frame: &mut Frame, t_ms: u64) {
        (**self).paint(frame, t_ms)
    }
    fn sync(&self, t_ms: u64, playing: bool, volume: f32) {
        (**self).sync(t_ms, playing, volume)
    }
}

/// Playback state. Keep one in your model and update it from [`MediaAction`]s.
/// The position is a pure function of the clock, so a playing stream needs no
/// per-frame messages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Playback {
    /// Position when `since` was recorded, in milliseconds.
    pub base_ms: u64,
    /// The `Ui::now()` reading at which playing started, or `None` when paused.
    pub since: Option<u64>,
    /// 0.0 to 1.0.
    pub volume: f32,
    /// Silenced without losing `volume`.
    pub muted: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            base_ms: 0,
            since: None,
            volume: 0.8,
            muted: false,
        }
    }
}

impl Playback {
    /// Paused at the start.
    pub fn new() -> Self {
        Self::default()
    }
    /// Current position at clock reading `now`, clamped to `duration`.
    pub fn position(&self, now: u64, duration: u64) -> u64 {
        let p = self.base_ms + self.since.map_or(0, |s| now.saturating_sub(s));
        p.min(duration)
    }
    /// True while playing.
    pub fn playing(&self) -> bool {
        self.since.is_some()
    }
    /// Start playing from the current position.
    pub fn play(&mut self, now: u64) {
        if self.since.is_none() {
            self.since = Some(now);
        }
    }
    /// Pause, keeping the position.
    pub fn pause(&mut self, now: u64, duration: u64) {
        self.base_ms = self.position(now, duration);
        self.since = None;
    }
    /// Jump to `ms`, staying paused or playing.
    pub fn seek(&mut self, now: u64, ms: u64) {
        self.base_ms = ms;
        if self.since.is_some() {
            self.since = Some(now);
        }
    }
    /// Apply an action (the usual body of `update`).
    pub fn apply(&mut self, action: MediaAction, now: u64, duration: u64) {
        match action {
            MediaAction::Toggle => {
                if self.playing() {
                    self.pause(now, duration);
                } else {
                    // Restart from the top when played at the end.
                    if self.position(now, duration) >= duration {
                        self.base_ms = 0;
                    }
                    self.play(now);
                }
            }
            MediaAction::Seek(ms) => self.seek(now, ms.min(duration)),
            MediaAction::Volume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                self.muted = false;
            }
            MediaAction::Mute => self.muted = !self.muted,
        }
    }
}

/// What the user did on the player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaAction {
    /// Play or pause.
    Toggle,
    /// Jump (or scrub) to this position in milliseconds.
    Seek(u64),
    /// Set the volume, 0.0 to 1.0.
    Volume(f32),
    /// Toggle mute.
    Mute,
}

type Handler<M> = Rc<dyn Fn(MediaAction) -> M>;

/// A picture area with a control bar. Click the picture or press Space to
/// play and pause, drag the bar to seek, Left and Right skip five seconds,
/// `m` mutes.
pub struct MediaPlayer<M> {
    ui: Ui,
    id: u64,
    source: Rc<dyn MediaSource>,
    state: Playback,
    title: Option<String>,
    on_action: Option<Handler<M>>,
    height: Length,
}

const BAR: f32 = 48.0;
const K_SCRUB: u64 = 0x3ED1_0001;
const K_VOL: u64 = 0x3ED1_0002;

impl<M: 'static> MediaPlayer<M> {
    /// A player for `source` showing `state`.
    pub fn new(ui: &Ui, id: u64, source: Rc<dyn MediaSource>, state: Playback) -> Self {
        Self {
            ui: ui.clone(),
            id,
            source,
            state,
            title: None,
            on_action: None,
            height: Length::Fixed(320.0),
        }
    }
    /// A name for the media, shown over the picture and read by screen readers.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    /// Receive controls as messages.
    pub fn on_action(mut self, f: impl Fn(MediaAction) -> M + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
    /// Total height including the control bar (default 320).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn emit(&self, out: &mut Vec<M>, a: MediaAction) {
        if let Some(f) = &self.on_action {
            out.push(f(a));
        }
    }
    fn picture(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            b.origin,
            Size::new(b.size.width, (b.size.height - BAR).max(0.0)),
        )
    }
    fn play_btn(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + 8.0, b.origin.y + b.size.height - BAR + 8.0),
            Size::new(32.0, 32.0),
        )
    }
    fn time_w(&self) -> f32 {
        let t = self.ui.theme();
        self.ui.metrics().width_styled(
            "00:00 / 00:00",
            t.typography.caption.size,
            400,
            FontFamily::Sans,
        ) + 4.0
    }
    fn seek_track(&self, b: &Rectangle) -> Rectangle {
        let x = b.origin.x + 8.0 + 32.0 + 12.0 + self.time_w() + 12.0;
        let right = b.origin.x + b.size.width - 8.0 - 28.0 - 8.0 - 72.0 - 12.0;
        Rectangle::new(
            Point::new(x, b.origin.y + b.size.height - BAR + (BAR - 16.0) / 2.0),
            Size::new((right - x).max(20.0), 16.0),
        )
    }
    fn vol_track(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(
                b.origin.x + b.size.width - 8.0 - 72.0,
                b.origin.y + b.size.height - BAR + (BAR - 16.0) / 2.0,
            ),
            Size::new(72.0, 16.0),
        )
    }
    fn mute_btn(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(
                b.origin.x + b.size.width - 8.0 - 72.0 - 8.0 - 28.0,
                b.origin.y + b.size.height - BAR + 10.0,
            ),
            Size::new(28.0, 28.0),
        )
    }
    fn ms_at(&self, b: &Rectangle, x: f32) -> u64 {
        let tr = self.seek_track(b);
        let f = ((x - tr.origin.x) / tr.size.width).clamp(0.0, 1.0);
        (f * self.source.duration_ms() as f32) as u64
    }
}

fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}", s / 60, s % 60)
}

impl<M: 'static> Widget<M> for MediaPlayer<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(
            320.0,
            match self.height {
                Length::Fixed(h) => h,
                _ => 240.0,
            },
        )
    }
    fn animating(&self) -> bool {
        self.state.playing() && !self.ui.reduced_motion()
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let dur = self.source.duration_ms();
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                if self.play_btn(&b).contains(cursor) || self.picture(&b).contains(cursor) {
                    self.emit(out, MediaAction::Toggle);
                } else if self.seek_track(&b).contains(cursor) {
                    self.ui.set_slot(self.id ^ K_SCRUB, Some(1));
                    self.emit(out, MediaAction::Seek(self.ms_at(&b, cursor.x)));
                } else if self.mute_btn(&b).contains(cursor) {
                    self.emit(out, MediaAction::Mute);
                } else if self.vol_track(&b).contains(cursor) {
                    self.ui.set_slot(self.id ^ K_VOL, Some(1));
                    let tr = self.vol_track(&b);
                    self.emit(
                        out,
                        MediaAction::Volume((cursor.x - tr.origin.x) / tr.size.width),
                    );
                }
                true
            }
            Event::CursorMoved(p) => {
                if self.ui.slot(self.id ^ K_SCRUB).is_some() {
                    self.emit(out, MediaAction::Seek(self.ms_at(&b, p.x)));
                    return true;
                }
                if self.ui.slot(self.id ^ K_VOL).is_some() {
                    let tr = self.vol_track(&b);
                    self.emit(
                        out,
                        MediaAction::Volume((p.x - tr.origin.x) / tr.size.width),
                    );
                    return true;
                }
                false
            }
            Event::MouseReleased(MouseButton::Left) => {
                let busy = self.ui.slot(self.id ^ K_SCRUB).is_some()
                    || self.ui.slot(self.id ^ K_VOL).is_some();
                self.ui.set_slot(self.id ^ K_SCRUB, None);
                self.ui.set_slot(self.id ^ K_VOL, None);
                busy
            }
            Event::KeyPressed(key, _) if self.ui.is_focused(Some(self.id)) => {
                let now = self.ui.now();
                let pos = self.state.position(now, dur);
                match key {
                    Key::Character(c) if c == " " => self.emit(out, MediaAction::Toggle),
                    Key::Character(c) if c == "m" => self.emit(out, MediaAction::Mute),
                    Key::Named(n) if n == "ArrowRight" => {
                        self.emit(out, MediaAction::Seek((pos + 5000).min(dur)))
                    }
                    Key::Named(n) if n == "ArrowLeft" => {
                        self.emit(out, MediaAction::Seek(pos.saturating_sub(5000)))
                    }
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.register(self.id);
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let b = layout.bounds;
        let cur = self.ui.cursor();
        let dur = self.source.duration_ms().max(1);
        let now = self.ui.now();
        let pos = self.state.position(now, dur);
        let playing = self.state.playing() && pos < dur;
        self.ui.describe(
            Role::Label,
            &format!(
                "{} player{}",
                if self.source.aspect().is_some() {
                    "Video"
                } else {
                    "Audio"
                },
                self.title
                    .as_ref()
                    .map(|t| format!(", {t}"))
                    .unwrap_or_default()
            ),
            &b,
            |_| {},
        );

        // Frame: black-ish letterbox with the picture aspect-fitted inside.
        let pic = self.picture(&b);
        let ink = Color::hex(0x101214);
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(b.origin, Size::new(b.size.width, b.size.height)),
            fill: p.surface,
            radius: t.radii.large,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(pic));
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(pic.origin.x + 1.0, pic.origin.y + 1.0),
                Size::new(pic.size.width - 2.0, pic.size.height - 1.0),
            ),
            fill: ink,
            radius: t.radii.large - 1.0,
            border: None,
            shadow: None,
        });
        let area = match self.source.aspect() {
            Some(a) if a > 0.0 => {
                let (mut w, mut h) = (pic.size.width - 2.0, pic.size.height - 2.0);
                if w / h > a {
                    w = h * a;
                } else {
                    h = w / a;
                }
                Rectangle::new(
                    Point::new(
                        pic.origin.x + (pic.size.width - w) / 2.0,
                        pic.origin.y + (pic.size.height - h) / 2.0,
                    ),
                    Size::new(w, h),
                )
            }
            _ => pic,
        };
        self.source.sync(
            pos,
            playing,
            if self.state.muted {
                0.0
            } else {
                self.state.volume
            },
        );
        let mut f = Frame::new(&self.ui, area.origin, area.size);
        self.source.paint(&mut f, pos);
        out.push(DrawCommand::PushClip(area));
        out.0.extend(f.finish());
        out.push(DrawCommand::PopClip);
        // Big play glyph while paused.
        if !playing {
            let c = Point::new(
                pic.origin.x + pic.size.width / 2.0,
                pic.origin.y + pic.size.height / 2.0,
            );
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(c.x - 28.0, c.y - 28.0), Size::new(56.0, 56.0)),
                fill: Color::hex(0x000000).with_alpha(0.55),
                radius: 28.0,
                border: Some(Border {
                    color: Color::hex(0xFFFFFF).with_alpha(0.4),
                    width: 1.0,
                }),
                shadow: None,
            });
            out.push(DrawCommand::Polygon {
                points: vec![
                    Point::new(c.x - 7.0, c.y - 12.0),
                    Point::new(c.x + 13.0, c.y),
                    Point::new(c.x - 7.0, c.y + 12.0),
                ],
                fill: Color::hex(0xFFFFFF),
            });
        }
        if let Some(title) = &self.title {
            out.push(label_cmd(
                Point::new(pic.origin.x + 14.0, pic.origin.y + 10.0),
                title.clone(),
                &cap,
                600,
                Color::hex(0xFFFFFF).with_alpha(0.85),
            ));
        }
        out.push(DrawCommand::PopClip);

        // Control bar.
        let bar_y = b.origin.y + b.size.height - BAR;
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(b.origin.x + 1.0, bar_y),
                Size::new(b.size.width - 2.0, 1.0),
            ),
            fill: p.border,
            radius: 0.0,
            border: None,
            shadow: None,
        });
        let pb = self.play_btn(&b);
        let hot = pb.contains(cur);
        out.push(DrawCommand::Quad {
            bounds: pb,
            fill: if hot { p.accent_hover } else { p.accent },
            radius: 16.0,
            border: None,
            shadow: None,
        });
        let c = Point::new(pb.origin.x + 16.0, pb.origin.y + 16.0);
        if playing {
            for dx in [-5.0, 1.5] {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(Point::new(c.x + dx, c.y - 6.0), Size::new(3.5, 12.0)),
                    fill: p.on_accent,
                    radius: 1.0,
                    border: None,
                    shadow: None,
                });
            }
        } else {
            out.push(DrawCommand::Polygon {
                points: vec![
                    Point::new(c.x - 4.0, c.y - 7.0),
                    Point::new(c.x + 7.0, c.y),
                    Point::new(c.x - 4.0, c.y + 7.0),
                ],
                fill: p.on_accent,
            });
        }
        if self.ui.is_focused(Some(self.id)) {
            out.push(util::focus_ring(pb, 16.0, p.focus_ring));
        }
        let time = format!("{} / {}", clock(pos), clock(dur));
        out.push(label_cmd(
            Point::new(
                pb.origin.x + 32.0 + 12.0,
                bar_y + (BAR - cap.line_height) / 2.0,
            ),
            time,
            &cap,
            400,
            p.text_secondary,
        ));
        // Seek track.
        let tr = self.seek_track(&b);
        let ty = tr.origin.y + tr.size.height / 2.0;
        let frac = pos as f32 / dur as f32;
        let scrubbing = self.ui.slot(self.id ^ K_SCRUB).is_some();
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(tr.origin.x, ty - 2.0),
                Size::new(tr.size.width, 4.0),
            ),
            fill: p.border_strong.with_alpha(0.4),
            radius: 2.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(tr.origin.x, ty - 2.0),
                Size::new((tr.size.width * frac).max(0.0), 4.0),
            ),
            fill: p.accent,
            radius: 2.0,
            border: None,
            shadow: None,
        });
        let knob_r = if scrubbing || tr.contains(cur) {
            7.0
        } else {
            5.5
        };
        let kx = tr.origin.x + tr.size.width * frac;
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(kx - knob_r, ty - knob_r),
                Size::new(2.0 * knob_r, 2.0 * knob_r),
            ),
            fill: p.surface_raised,
            radius: knob_r,
            border: Some(Border {
                color: p.accent,
                width: 2.0,
            }),
            shadow: Some(t.elevation.low),
        });
        // Mute button: a speaker drawn from a polygon and arcs.
        let mb = self.mute_btn(&b);
        if mb.contains(cur) {
            out.push(DrawCommand::Quad {
                bounds: mb,
                fill: p.hover,
                radius: t.radii.medium,
                border: None,
                shadow: None,
            });
        }
        let m = Point::new(mb.origin.x + 14.0, mb.origin.y + 14.0);
        let ink_c = p.text_secondary;
        out.push(DrawCommand::Polygon {
            points: vec![
                Point::new(m.x - 8.0, m.y - 3.0),
                Point::new(m.x - 4.0, m.y - 3.0),
                Point::new(m.x + 1.0, m.y - 7.0),
                Point::new(m.x + 1.0, m.y + 7.0),
                Point::new(m.x - 4.0, m.y + 3.0),
                Point::new(m.x - 8.0, m.y + 3.0),
            ],
            fill: ink_c,
        });
        if self.state.muted || self.state.volume <= 0.0 {
            out.push(DrawCommand::Line {
                from: Point::new(m.x + 4.0, m.y - 4.0),
                to: Point::new(m.x + 10.0, m.y + 4.0),
                width: 1.6,
                color: ink_c,
            });
            out.push(DrawCommand::Line {
                from: Point::new(m.x + 10.0, m.y - 4.0),
                to: Point::new(m.x + 4.0, m.y + 4.0),
                width: 1.6,
                color: ink_c,
            });
        } else {
            for (k, r) in [4.0f32, 8.0].into_iter().enumerate() {
                if self.state.volume > 0.5 * k as f32 {
                    let mut prev: Option<Point> = None;
                    for i in 0..=8 {
                        let a = -0.9 + 1.8 * i as f32 / 8.0;
                        let q = Point::new(m.x + 1.0 + r * a.cos(), m.y + r * a.sin());
                        if let Some(pq) = prev {
                            out.push(DrawCommand::Line {
                                from: pq,
                                to: q,
                                width: 1.5,
                                color: ink_c,
                            });
                        }
                        prev = Some(q);
                    }
                }
            }
        }
        // Volume track.
        let vt = self.vol_track(&b);
        let vy = vt.origin.y + vt.size.height / 2.0;
        let v = if self.state.muted {
            0.0
        } else {
            self.state.volume
        };
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(vt.origin.x, vy - 2.0),
                Size::new(vt.size.width, 4.0),
            ),
            fill: p.border_strong.with_alpha(0.4),
            radius: 2.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(vt.origin.x, vy - 2.0),
                Size::new(vt.size.width * v, 4.0),
            ),
            fill: p.text_secondary,
            radius: 2.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(vt.origin.x + vt.size.width * v - 5.0, vy - 5.0),
                Size::new(10.0, 10.0),
            ),
            fill: p.surface_raised,
            radius: 5.0,
            border: Some(Border {
                color: p.text_secondary,
                width: 1.5,
            }),
            shadow: None,
        });
    }
}

/// A procedural demo source: colour bars with a sweeping scanline and a
/// running clock (video), or an animated level meter (audio). Use it to try the
/// player without a decoder, or as a template for your own [`MediaSource`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestCard {
    duration_ms: u64,
    audio: bool,
}

impl TestCard {
    /// A 16:9 picture lasting `seconds`.
    pub fn video(seconds: u64) -> Self {
        Self {
            duration_ms: seconds * 1000,
            audio: false,
        }
    }
    /// A level-meter visualiser lasting `seconds`.
    pub fn audio(seconds: u64) -> Self {
        Self {
            duration_ms: seconds * 1000,
            audio: true,
        }
    }
}

impl MediaSource for TestCard {
    fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    fn aspect(&self) -> Option<f32> {
        (!self.audio).then_some(16.0 / 9.0)
    }
    fn paint(&self, f: &mut Frame, t_ms: u64) {
        let s = f.size();
        let full = Rectangle::new(Point::default(), s);
        if self.audio {
            // Deterministic pseudo-levels that move with time.
            let bars = 28;
            let w = (s.width - 48.0) / bars as f32;
            for i in 0..bars {
                let ph = t_ms as f32 / 1000.0 * 3.0 + i as f32 * 0.55;
                let lvl =
                    0.25 + 0.5 * (ph.sin() * 0.5 + 0.5) * (1.0 - (i as f32 / bars as f32) * 0.5);
                let h = (s.height * 0.6 * lvl).max(4.0);
                let c = f.palette().accent.with_alpha(0.35 + 0.65 * lvl);
                f.round_rect(
                    Rectangle::new(
                        Point::new(24.0 + i as f32 * w + 2.0, s.height * 0.75 - h),
                        Size::new((w - 4.0).max(2.0), h),
                    ),
                    2.0,
                    c,
                );
            }
            return;
        }
        let bars = [
            0xC8C8C8u32,
            0xC8C84A,
            0x4AC8C8,
            0x4AC84A,
            0xC84AC8,
            0xC84A4A,
            0x4A4AC8,
        ];
        let bw = s.width / bars.len() as f32;
        for (i, c) in bars.iter().enumerate() {
            f.rect(
                Rectangle::new(
                    Point::new(i as f32 * bw, 0.0),
                    Size::new(bw + 0.5, s.height * 0.72),
                ),
                Color::hex(*c),
            );
        }
        let low = [0x1D1D70u32, 0x131313, 0x4B0082, 0x131313];
        for (i, c) in low.iter().enumerate() {
            f.rect(
                Rectangle::new(
                    Point::new(i as f32 * s.width / 4.0, s.height * 0.72),
                    Size::new(s.width / 4.0 + 0.5, s.height * 0.28),
                ),
                Color::hex(*c),
            );
        }
        // Scanline sweeps once every four seconds.
        let x = (t_ms % 4000) as f32 / 4000.0 * s.width;
        f.rect(
            Rectangle::new(Point::new(x - 1.5, 0.0), Size::new(3.0, s.height)),
            Color::hex(0xFFFFFF).with_alpha(0.9),
        );
        let label = format!(
            "{:02}:{:02}.{}",
            t_ms / 60000,
            t_ms / 1000 % 60,
            t_ms / 100 % 10
        );
        let size = (s.height * 0.1).max(12.0);
        let w = f.text_width(&label, size) + 24.0;
        let r = Rectangle::new(
            Point::new((s.width - w) / 2.0, s.height * 0.8 - size),
            Size::new(w, size * 2.0),
        );
        f.round_rect(r, 6.0, Color::hex(0x101214).with_alpha(0.85));
        f.text_weight(
            Point::new(r.origin.x + 12.0, r.origin.y + size * 0.4),
            label,
            size,
            600,
            Color::hex(0xFFFFFF),
        );
        let _ = full;
    }
}

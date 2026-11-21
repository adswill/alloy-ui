//! Vector icons from the bundled Lucide and Phosphor packs.
//!
//! Icons are glyphs of two small bundled fonts, so they antialias, scale and take their colour
//! exactly like text, on every renderer, with no extra draw primitive. The licences (ISC and
//! MIT) sit next to the font files in `crates/render/fonts`.

pub use crate::icon_data::Icon;
use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, FontFamily, Layout, Point, Size, Widget,
};

/// Which bundled icon pack to draw from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IconPack {
    /// Lucide: 24 px grid, 2 px round strokes, crisp and neutral (the default).
    #[default]
    Lucide,
    /// Phosphor Regular: slightly softer, a little more character.
    Phosphor,
}

impl IconPack {
    /// The font that holds this pack.
    pub const fn family(self) -> FontFamily {
        match self {
            IconPack::Lucide => FontFamily::Lucide,
            IconPack::Phosphor => FontFamily::Phosphor,
        }
    }
    /// The glyph for `icon` in this pack.
    pub const fn glyph(self, icon: Icon) -> char {
        match self {
            IconPack::Lucide => icon.lucide(),
            IconPack::Phosphor => icon.phosphor(),
        }
    }
}

/// One icon, square, in the text colour unless told otherwise. Size defaults to 16 px.
pub struct IconView {
    ui: Ui,
    icon: Icon,
    pack: Option<IconPack>,
    size: f32,
    color: Option<Color>,
}

impl IconView {
    /// `icon` in the window's icon pack.
    pub fn new(ui: &Ui, icon: Icon) -> Self {
        Self {
            ui: ui.clone(),
            icon,
            pack: None,
            size: 16.0,
            color: None,
        }
    }
    /// Edge length in logical pixels.
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// Colour override.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Secondary (de-emphasised) colour.
    pub fn secondary(mut self) -> Self {
        self.color = Some(self.ui.theme().palette.text_secondary);
        self
    }
    /// Force a pack instead of the window default.
    pub fn pack(mut self, pack: IconPack) -> Self {
        self.pack = Some(pack);
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for IconView {
    fn style(&self) -> Style {
        util::sized(
            alloy_ui_layout::Length::Fixed(self.size),
            alloy_ui_layout::Length::Fixed(self.size),
        )
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(self.size, self.size)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        if self.ui.culled(&b) {
            return;
        }
        let pack = self.pack.unwrap_or_else(|| self.ui.icon_pack());
        out.push(DrawCommand::Text {
            origin: Point::new(b.origin.x, b.origin.y),
            content: pack.glyph(self.icon).to_string(),
            size: self.size,
            color: self.color.unwrap_or(self.ui.theme().palette.text),
            max_width: None,
            weight: 400,
            family: pack.family(),
            line_height: Some(self.size),
        });
    }
}

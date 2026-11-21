//! Icons: `icon(Icon::Search)`, from the bundled Lucide and Phosphor packs.

use crate::{cx, view::Node};
use alloy_ui_core::Color;
use alloy_ui_widgets as w;

pub use w::{Icon, IconPack};

/// An icon under construction. See [`icon`].
pub struct Ico {
    icon: Icon,
    size: f32,
    dim: bool,
    color: Option<Color>,
    pack: Option<IconPack>,
}

/// One icon, 16 px in the text colour: `icon(Icon::Search)`, `icon(Icon::Bell).size(20.0).dim()`.
/// Which pack it comes from is set once for the window with [`set_icon_pack`].
pub fn icon(icon: Icon) -> Ico {
    Ico {
        icon,
        size: 16.0,
        dim: false,
        color: None,
        pack: None,
    }
}

/// Choose the icon pack for the whole window: `Lucide` (default) or `Phosphor`.
pub fn set_icon_pack(pack: IconPack) {
    cx::ui().set_icon_pack(pack);
}

impl Ico {
    /// Edge length in pixels.
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// Secondary colour.
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
    /// Explicit colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// The theme accent.
    pub fn accent(self) -> Self {
        let c = cx::palette().accent;
        self.color(c)
    }
    /// Force one pack regardless of the window default.
    pub fn pack(mut self, pack: IconPack) -> Self {
        self.pack = Some(pack);
        self
    }
}

impl<M: 'static> From<Ico> for Node<M> {
    fn from(i: Ico) -> Self {
        let mut v = w::IconView::new(&cx::ui(), i.icon).size(i.size);
        if i.dim {
            v = v.secondary();
        }
        if let Some(c) = i.color {
            v = v.color(c);
        }
        if let Some(p) = i.pack {
            v = v.pack(p);
        }
        Node(v.into_element())
    }
}

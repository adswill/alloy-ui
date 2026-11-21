//! Alloy core: the shared contracts every other crate builds on.
//!
//! Architecture is Elm-style: `State`, `Message`, `update`, `view`.
//! Nothing here touches a window system or a GPU.
#![warn(missing_docs)]

pub mod draw;
pub mod event;
pub mod geometry;
pub mod inspector;
pub mod runtime;
pub mod style;
pub mod widget;

pub use draw::{Border, DrawCommand, DrawList, FontFamily, Shadow};
pub use event::{Event, Key, Modifiers, MouseButton};
pub use geometry::{Color, Point, Rectangle, Size, Vector};
pub use runtime::{
    Application, Command, Layouter, Outcome, Runtime, Subscription, SubscriptionKind, WindowId,
    WindowRequest, WindowSpec,
};
pub use widget::{Element, Layout, Widget};

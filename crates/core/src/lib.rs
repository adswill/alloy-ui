//! Alloy core: the shared contracts every other crate builds on.
//!
//! Architecture is Elm-style: `State`, `Message`, `update`, `view`.
//! Nothing here touches a window system or a GPU.
#![warn(missing_docs)]

pub mod draw;
pub mod event;
pub mod geometry;
pub mod runtime;
pub mod style;
pub mod widget;

pub use draw::{Border, DrawCommand, DrawList, Shadow};
pub use event::{Event, Key, Modifiers, MouseButton};
pub use geometry::{Color, Point, Rectangle, Size, Vector};
pub use runtime::{Application, Command, Subscription};
pub use widget::{Element, Layout, Widget};

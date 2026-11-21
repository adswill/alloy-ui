//! Alloy core: the shared contracts every other crate builds on.
//!
//! Architecture is Elm-style: `State`, `Message`, `update`, `view`.
//! Nothing here touches a window system or a GPU.
#![warn(missing_docs)]

pub mod draw;
pub mod event;
pub mod geometry;
pub mod host;
pub mod inspector;
pub mod runtime;
pub mod style;
pub mod widget;

pub use draw::{Border, DrawCommand, DrawList, FontFamily, Shadow};
pub use event::{Event, Key, Modifiers, MouseButton};
pub use geometry::{Color, Point, Rectangle, Size, Vector};
pub use host::{HostConfig, HostEvent, WindowEffect};
pub use runtime::{
    Application, Command, Layouter, Outcome, Runtime, Subscription, TaskRequest, SubscriptionKind, WindowId,
    WindowRequest, WindowSpec,
};
pub use widget::{Element, Layout, Widget};

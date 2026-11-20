//! Alloy built-in widgets: text, button, checkbox, slider, text_input,
//! scrollable, list, row, column and container.
//!
//! # State model
//!
//! Widgets are rebuilt on every `view`, so anything that must survive a
//! rebuild (focus, caret, scroll offset, which button is held down) lives in a
//! [`Ui`] handle. Create one `Ui` per window, keep it in your application
//! state, pass `&ui` to every widget constructor, and wrap the root element in
//! [`Root`]. `Root` owns Tab navigation and cursor tracking.
//!
//! Widgets never see a renderer: they measure text through
//! [`TextMetrics`] (an approximation until the renderer provides real
//! metrics) and emit `alloy_ui_core::DrawCommand`s.
#![warn(missing_docs)]

mod button;
mod checkbox;
mod container;
mod flex;
mod list;
mod scrollable;
mod slider;
mod text;
mod text_edit;
mod text_input;
mod ui;
mod util;

pub use button::{Button, Variant};
pub use checkbox::Checkbox;
pub use container::Container;
pub use flex::{Column, Row};
pub use list::List;
pub use scrollable::Scrollable;
pub use slider::Slider;
pub use text::{Text, TextRole};
pub use text_edit::Edit;
pub use text_input::TextInput;
pub use ui::{ApproxMetrics, Clipboard, MemoryClipboard, Root, TextMetrics, Ui};
pub use util::arrange_widget;

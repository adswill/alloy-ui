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

mod a11y;
mod button;
mod chart;
mod checkbox;
mod container;
mod date_picker;
mod flex;
mod forced;
mod image;
mod list;
mod motion;
mod number_input;
mod pick_list;
mod progress;
mod radio;
mod reveal;
mod scrollable;
mod segmented;
mod slider;
mod space;
mod table;
mod text;
mod text_edit;
mod text_editor;
mod text_input;
mod toggle;
mod tooltip;
mod ui;
mod util;

pub use a11y::{A11yNode, Role};
pub use button::{Button, Variant};
pub use chart::BarChart;
pub use checkbox::Checkbox;
pub use container::Container;
pub use date_picker::{days_in_month, Date, DatePicker};
pub use flex::{Column, Row};
pub use forced::Forced;
pub use image::Image;
pub use list::List;
pub use motion::{Animatable, Presence, Spring, Transition};
pub use number_input::NumberInput;
pub use pick_list::PickList;
pub use progress::ProgressBar;
pub use radio::Radio;
pub use reveal::Reveal;
pub use scrollable::Scrollable;
pub use segmented::Segmented;
pub use slider::Slider;
pub use space::{Rule, Space};
pub use table::{Table, TableColumn};
pub use text::{Text, TextRole};
pub use text_edit::Edit;
pub use text_editor::TextEditor;
pub use text_input::TextInput;
pub use toggle::Toggle;
pub use tooltip::Tooltip;
pub use ui::{ApproxMetrics, Clipboard, MemoryClipboard, Root, TextMetrics, Ui};
pub use util::arrange_widget;

/// Lay out a view tree at window size `size`: the `Layouter` the runtime needs.
///
/// ```ignore
/// alloy_ui::run(settings, app, alloy_ui_widgets::layout::<Message>)
/// ```
pub fn layout<M: 'static>(
    root: &alloy_ui_core::Element<M>,
    size: alloy_ui_core::Size,
) -> alloy_ui_core::Layout {
    arrange_widget(
        root.0.as_ref(),
        alloy_ui_core::Rectangle::new(alloy_ui_core::Point::default(), size),
    )
}

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
mod badge;
mod button;
mod chart;
mod checkbox;
mod code_editor;
mod color_picker;
mod container;
mod data_grid;
mod date_picker;
mod flex;
mod forced;
mod highlight;
mod image;
mod list;
mod markdown;
mod menu;
mod motion;
mod number_input;
mod pick_list;
mod plot;
mod popover;
mod progress;
mod radio;
mod reveal;
mod rich_editor;
mod rich_text;
mod scrollable;
mod segmented;
mod slider;
mod space;
mod split;
mod swatch;
mod table;
mod tabs;
mod text;
mod text_edit;
mod text_editor;
mod text_input;
mod toggle;
mod tooltip;
mod tree;
mod ui;
mod util;
mod virtual_list;

pub use a11y::{A11yNode, Role};
pub use badge::{Badge, Tone};
pub use button::{Button, Variant};
pub use chart::BarChart;
pub use checkbox::Checkbox;
pub use code_editor::CodeEditor;
pub use color_picker::{from_hsv, parse_hex, to_hsv, ColorPicker};
pub use container::Container;
pub use data_grid::{DataGrid, GridCell, GridColumn, Selection};
pub use date_picker::{days_in_month, Date, DatePicker};
pub use flex::{Column, Row};
pub use forced::Forced;
pub use highlight::{tokenize, Lang, Span, State as LexState, Token};
pub use image::Image;
pub use list::List;
pub use markdown::Markdown;
pub use menu::{ContextMenu, MenuItem};
pub use motion::{Animatable, Presence, Spring, Transition};
pub use number_input::NumberInput;
pub use pick_list::PickList;
pub use plot::{format_exact, format_value, nice_ticks, series_color, Chart, Series, Sparkline};
pub use popover::Popover;
pub use progress::ProgressBar;
pub use radio::Radio;
pub use reveal::Reveal;
pub use rich_editor::RichEditor;
pub use rich_text::{Block, BlockKind, Mark, Marks, RichText, Run};
pub use scrollable::Scrollable;
pub use segmented::Segmented;
pub use slider::Slider;
pub use space::{Rule, Space};
pub use split::SplitPane;
pub use swatch::{hex_of, Swatch};
pub use table::{Table, TableColumn};
pub use tabs::Tabs;
pub use text::{Text, TextRole};
pub use text_edit::Edit;
pub use text_editor::TextEditor;
pub use text_input::TextInput;
pub use toggle::Toggle;
pub use tooltip::Tooltip;
pub use tree::{Tree, TreeItem};
pub use ui::{ApproxMetrics, Clipboard, MemoryClipboard, Root, TextMetrics, Ui};
pub use util::arrange_widget;
pub use virtual_list::VirtualList;

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

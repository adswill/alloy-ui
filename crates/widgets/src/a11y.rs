//! Accessibility tree: what each widget is, called and currently holds.
//!
//! Widgets describe themselves while drawing, so after a frame
//! [`Ui::accessibility`](crate::Ui::accessibility) returns the nodes in draw
//! (reading) order. A platform adapter maps them to AccessKit, NSAccessibility,
//! UIA or AT-SPI; widgets never know which.

use alloy_ui_core::Rectangle;

/// What kind of control a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Static text.
    Label,
    /// Push button.
    Button,
    /// Two-state box.
    Checkbox,
    /// Switch.
    Switch,
    /// One choice of a group.
    Radio,
    /// Range control.
    Slider,
    /// Numeric stepper.
    SpinButton,
    /// Single-line editable text.
    TextInput,
    /// Multi-line editable text.
    TextEditor,
    /// Drop-down.
    ComboBox,
    /// Calendar field.
    DatePicker,
    /// A segment of a segmented control.
    Tab,
    /// Progress indicator.
    ProgressBar,
}

/// One accessible element.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yNode {
    /// Control kind.
    pub role: Role,
    /// Accessible name (button text, field label, ...).
    pub label: String,
    /// Current value as text (field contents, slider reading, ...).
    pub value: Option<String>,
    /// Toggle or selection state, where it applies.
    pub checked: Option<bool>,
    /// Focus id, if the widget is keyboard-focusable.
    pub id: Option<u64>,
    /// Whether it currently has keyboard focus.
    pub focused: bool,
    /// False when the widget has no handler.
    pub enabled: bool,
    /// Where it is, in window coordinates.
    pub bounds: Rectangle,
}

impl A11yNode {
    /// A node of `role` named `label`, enabled and unfocused.
    pub(crate) fn new(role: Role, label: impl Into<String>, bounds: Rectangle) -> Self {
        Self {
            role,
            label: label.into(),
            value: None,
            checked: None,
            id: None,
            focused: false,
            enabled: true,
            bounds,
        }
    }
}

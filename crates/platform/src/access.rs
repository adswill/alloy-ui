//! Accessibility: the platform-neutral tree a screen reader sees, and the
//! actions it can send back.
//!
//! The application describes each frame as an [`AccessTree`] (one flat list of
//! [`AccessNode`]s in reading order). The shell hands it to the OS adapter for
//! that window (AT-SPI2 on Linux, NSAccessibility on macOS, UI Automation on
//! Windows), which publishes only what changed. Assistive technology can ask
//! for things (press a button, move focus, set a value); those come back as
//! [`PlatformEvent::Accessibility`](crate::PlatformEvent::Accessibility).
//!
//! Node ids belong to the caller and must stay the same for the same widget
//! from frame to frame, or screen readers will think the control was replaced
//! and re-announce it. Use the widget's focus id where it has one.

use alloy_ui_core::Rectangle;

/// Stable identity of one accessible element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccessId(pub u64);

/// What kind of control a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRole {
    /// Static text.
    Label,
    /// Push button.
    Button,
    /// Two-state box.
    Checkbox,
    /// On/off switch.
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
    /// One tab or segment.
    Tab,
    /// Progress indicator.
    ProgressBar,
    /// Anything else (a plain container).
    Group,
}

/// One accessible element.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessNode {
    /// Stable id.
    pub id: AccessId,
    /// Control kind.
    pub role: AccessRole,
    /// Accessible name.
    pub label: String,
    /// Current value as text.
    pub value: Option<String>,
    /// Toggle state, where it applies.
    pub checked: Option<bool>,
    /// Whether it has keyboard focus.
    pub focused: bool,
    /// False when the control is disabled.
    pub enabled: bool,
    /// Position in window coordinates, logical pixels.
    pub bounds: Rectangle,
    /// Limits for sliders, spin buttons and progress bars. The current value
    /// is read from [`AccessNode::value`] as a number. When `None`, adapters
    /// assume 0 to 100 in steps of 1.
    pub range: Option<AccessRange>,
}

/// Numeric limits of a range control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccessRange {
    /// Smallest value.
    pub min: f64,
    /// Largest value.
    pub max: f64,
    /// Smallest useful change.
    pub step: f64,
}

/// Everything one window exposes. Nodes are children of the window, in
/// reading order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessTree {
    /// Window title (the name of the top-level element).
    pub title: String,
    /// The elements.
    pub nodes: Vec<AccessNode>,
}

/// What assistive technology asks a control to do.
#[derive(Debug, Clone, PartialEq)]
pub enum AccessAction {
    /// Activate (click, toggle, select).
    Press,
    /// Move keyboard focus here.
    Focus,
    /// Replace the value (text fields, numeric fields).
    SetValue(String),
    /// Step a slider or spin button up.
    Increment,
    /// Step a slider or spin button down.
    Decrement,
}

/// An action aimed at one node.
#[derive(Debug, Clone, PartialEq)]
pub struct AccessRequest {
    /// The target node.
    pub target: AccessId,
    /// What to do.
    pub action: AccessAction,
}

/// Where adapters deliver [`AccessRequest`]s; callable from any thread.
pub type ActionSink = std::sync::Arc<dyn Fn(AccessRequest) + Send + Sync>;

/// One window's connection to the OS accessibility service.
pub trait AccessAdapter {
    /// Publish a new tree. Adapters send only the difference to the last one.
    fn update(&mut self, tree: &AccessTree);
}

/// Difference between two trees, by node id.
#[derive(Debug, Default, PartialEq)]
pub struct TreeDiff {
    /// Ids present only in the new tree.
    pub added: Vec<AccessId>,
    /// Ids present only in the old tree.
    pub removed: Vec<AccessId>,
    /// Ids whose name, value, checked state, focus, enabled flag or bounds changed.
    pub changed: Vec<AccessId>,
    /// True if the node order (reading order) differs for surviving nodes.
    pub reordered: bool,
}

impl TreeDiff {
    /// True when nothing differs.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && !self.reordered
    }
}

impl AccessTree {
    /// The node with this id.
    pub fn get(&self, id: AccessId) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The focused node, if any.
    pub fn focused(&self) -> Option<&AccessNode> {
        self.nodes.iter().find(|n| n.focused)
    }

    /// Compare with a newer tree.
    pub fn diff(&self, new: &AccessTree) -> TreeDiff {
        use std::collections::HashMap;
        let old: HashMap<AccessId, &AccessNode> = self.nodes.iter().map(|n| (n.id, n)).collect();
        let cur: HashMap<AccessId, &AccessNode> = new.nodes.iter().map(|n| (n.id, n)).collect();
        let mut d = TreeDiff::default();
        for n in &new.nodes {
            match old.get(&n.id) {
                None => d.added.push(n.id),
                Some(o) if *o != n => d.changed.push(n.id),
                Some(_) => {}
            }
        }
        d.removed = self
            .nodes
            .iter()
            .filter(|n| !cur.contains_key(&n.id))
            .map(|n| n.id)
            .collect();
        let kept_old = self.nodes.iter().filter(|n| cur.contains_key(&n.id));
        let kept_new = new.nodes.iter().filter(|n| old.contains_key(&n.id));
        d.reordered = kept_old.map(|n| n.id).ne(kept_new.map(|n| n.id));
        d
    }
}

/// Create an AT-SPI adapter for window `key` without a real window, for
/// tests that drive the Linux backend directly.
#[cfg(target_os = "linux")]
#[doc(hidden)]
pub fn atspi_adapter_for_tests(key: u64, sink: ActionSink) -> Box<dyn AccessAdapter> {
    let origin: crate::os::linux_atspi::Origin =
        std::sync::Arc::new(|| crate::os::linux_atspi::ScreenOrigin {
            x: 100,
            y: 50,
            scale: 1.0,
        });
    Box::new(crate::os::linux_atspi::AtspiAdapter::new(key, origin, sink))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u64, label: &str) -> AccessNode {
        AccessNode {
            id: AccessId(id),
            role: AccessRole::Button,
            label: label.into(),
            value: None,
            checked: None,
            focused: false,
            enabled: true,
            bounds: Rectangle::default(),
            range: None,
        }
    }

    fn tree(nodes: Vec<AccessNode>) -> AccessTree {
        AccessTree {
            title: "t".into(),
            nodes,
        }
    }

    #[test]
    fn identical_trees_have_empty_diff() {
        let a = tree(vec![node(1, "a"), node(2, "b")]);
        assert!(a.diff(&a.clone()).is_empty());
    }

    #[test]
    fn diff_finds_added_removed_changed() {
        let a = tree(vec![node(1, "a"), node(2, "b")]);
        let mut b = tree(vec![node(2, "B"), node(3, "c")]);
        b.nodes[0].focused = true;
        let d = a.diff(&b);
        assert_eq!(d.added, [AccessId(3)]);
        assert_eq!(d.removed, [AccessId(1)]);
        assert_eq!(d.changed, [AccessId(2)]);
        assert!(!d.reordered);
    }

    #[test]
    fn diff_detects_reorder() {
        let a = tree(vec![node(1, "a"), node(2, "b")]);
        let b = tree(vec![node(2, "b"), node(1, "a")]);
        assert!(a.diff(&b).reordered);
    }
}

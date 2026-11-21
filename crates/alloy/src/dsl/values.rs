//! Value widgets: inputs, sliders, radios, tabs and pickers that write into state.

use super::*;

/// A lens into the state: which field a widget edits.
pub(super) type Lens<S, T> = Arc<dyn for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync>;

pub(super) fn set_via<S: 'static, T: Send + Sync + Clone + 'static>(
    lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
) -> impl Fn(T) -> Act<S> {
    let lens: Lens<S, T> = Arc::new(lens);
    move |v| {
        let lens = lens.clone();
        Act::new(move |s| *lens(s) = v.clone())
    }
}

pub(super) fn set_with<S: 'static, T: Send + Sync + Clone + 'static>(
    f: impl Fn(&mut S, T) + Send + Sync + 'static,
) -> impl Fn(T) -> Act<S> {
    let f = Arc::new(f);
    move |v| {
        let f = f.clone();
        Act::new(move |s| f(s, v.clone()))
    }
}

/// A single-line text field. See [`input`].
pub struct Input<M> {
    value: String,
    hint: String,
    on: Option<Box<dyn Fn(String) -> M>>,
    enter: Option<Box<dyn Fn() -> M>>,
    id: u64,
    wide: bool,
    px: Option<f32>,
    secure: bool,
}

/// A single-line text field showing `value`.
#[track_caller]
pub fn input<M>(value: impl Val<String, M>) -> Input<M> {
    let (value, on) = value.split();
    Input {
        value,
        hint: String::new(),
        on,
        enter: None,
        id: auto_id(2),
        wide: false,
        px: None,
        secure: false,
    }
}
impl<M> Input<M> {
    /// Placeholder shown while empty.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    /// Stretch to the available width.
    pub fn fill_w(mut self) -> Self {
        self.wide = true;
        self
    }
    /// A fixed width in pixels.
    pub fn w(mut self, px: f32) -> Self {
        self.px = Some(px);
        self
    }
    /// Password field: bullets instead of characters, no copy or cut.
    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }
    /// Message sent when Enter is pressed.
    pub fn enter(mut self, m: M) -> Self
    where
        M: Clone + 'static,
    {
        self.enter = Some(Box::new(move || m.clone()));
        self
    }
    /// Explicit id.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
}
impl<S: 'static> Input<Act<S>> {
    /// Closure style: run `f` on the state when Enter is pressed.
    pub fn submit(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.enter(Act::new(f))
    }
}
value_widget!(
    Input,
    "input",
    String,
    "Called with the new text on every edit."
);
impl<M: 'static> From<Input<M>> for Node<M> {
    fn from(i: Input<M>) -> Self {
        let mut t = w::TextInput::new(&cx::ui(), i.id, i.value).placeholder(i.hint);
        if let Some(f) = i.on {
            t = t.on_input(f);
        }
        if let Some(f) = i.enter {
            t = t.on_submit(f);
        }
        t = t.secure(i.secure);
        if i.wide {
            t = t.width(Length::Fill(1));
        }
        if let Some(px) = i.px {
            t = t.width(Length::Fixed(px));
        }
        Node(t.into_element())
    }
}

/// A multi-line text area. See [`editor`].
pub struct Editor<M> {
    value: String,
    hint: String,
    on: Option<Box<dyn Fn(String) -> M>>,
    id: u64,
}
/// A multi-line text area that fills the space it is given.
#[track_caller]
pub fn editor<M>(value: impl Val<String, M>) -> Editor<M> {
    let (value, on) = value.split();
    Editor {
        value,
        hint: String::new(),
        on,
        id: auto_id(3),
    }
}
impl<M> Editor<M> {
    /// Use `id` for the editor's cursor and scroll state. Give each document its own when one
    /// editor shows different texts, so they do not share a cursor.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
    /// Placeholder shown while empty.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
}
value_widget!(
    Editor,
    "editor",
    String,
    "Called with the whole text on every edit."
);
impl<M: 'static> From<Editor<M>> for Node<M> {
    fn from(e: Editor<M>) -> Self {
        let mut t = w::TextEditor::new(&cx::ui(), e.id, e.value)
            .placeholder(e.hint)
            .width(Length::Fill(1))
            .height(Length::Fill(1));
        if let Some(f) = e.on {
            t = t.on_input(f);
        }
        Node(t.into_element())
    }
}

macro_rules! switch {
    ($name:ident, $fnname:ident, $widget:ident, $salt:literal, $doc:literal) => {
        #[doc = $doc]
        pub struct $name<M> {
            label: String,
            value: bool,
            on: Option<Box<dyn Fn(bool) -> M>>,
            id: u64,
        }
        #[doc = $doc]
        #[track_caller]
        pub fn $fnname<M>(label: impl Into<String>, value: impl Val<bool, M>) -> $name<M> {
            let (value, on) = value.split();
            $name {
                label: label.into(),
                value,
                on,
                id: auto_id($salt),
            }
        }
        value_widget!($name, "", bool, "Called with the requested new value.");
        impl<M: 'static> From<$name<M>> for Node<M> {
            fn from(c: $name<M>) -> Self {
                let mut t = w::$widget::new(&cx::ui(), c.label, c.value).id(c.id);
                if let Some(f) = c.on {
                    t = t.on_toggle(f);
                }
                Node(t.into_element())
            }
        }
    };
}
switch!(Check, check, Checkbox, 4, "A labelled checkbox.");
switch!(Switch, toggle, Toggle, 5, "A labelled on/off switch.");

/// A radio option. See [`radio`].
pub struct Radio<M> {
    label: String,
    on: Option<M>,
    selected: bool,
    id: u64,
}
/// A radio option, `selected` when it is the current choice.
#[track_caller]
pub fn radio<M>(label: impl Into<String>, selected: bool) -> Radio<M> {
    Radio {
        label: label.into(),
        on: None,
        selected,
        id: auto_id(6),
    }
}
impl<M> Radio<M> {
    /// Message sent when chosen.
    pub fn on(mut self, m: M) -> Self {
        self.on = Some(m);
        self
    }
}
impl<M: Clone + 'static> From<Radio<M>> for Node<M> {
    fn from(r: Radio<M>) -> Self {
        let mut t = w::Radio::new(&cx::ui(), r.label, r.selected).id(r.id);
        if let Some(m) = r.on {
            t = t.on_select(m);
        }
        Node(t.into_element())
    }
}

/// A column of radio options bound to a field: picking one writes its value there.
///
/// ```ignore
/// radios(at!(s.plan), [("Free", Plan::Free), ("Pro", Plan::Pro)])
/// ```
#[track_caller]
pub fn radios<S: 'static, T, L: Into<String>>(
    place: Place<'_, S, T>,
    options: impl IntoIterator<Item = (L, T)>,
) -> crate::view::Flex<Act<S>>
where
    T: PartialEq + Clone + Send + Sync + 'static,
{
    let mut col = crate::view::col::<Act<S>, Node<Act<S>>>([]);
    for (label, v) in options {
        let on = *place.cur == v;
        col = col.push(radio(label, on).on(place.writer(v)));
    }
    col
}

/// Radios for every variant of a [`Choice`] enum, bound to a field: `radio_list(at!(s.plan))`.
#[track_caller]
pub fn radio_list<S: 'static, T: Choice + Send + Sync>(
    place: Place<'_, S, T>,
) -> crate::view::Flex<Act<S>> {
    radio_list_with(place, |_| String::new())
}

/// Like [`radio_list`], with a note after each label: `radio_list_with(at!(s.plan), price)`.
#[track_caller]
pub fn radio_list_with<S: 'static, T: Choice + Send + Sync>(
    place: Place<'_, S, T>,
    note: impl Fn(T) -> String,
) -> crate::view::Flex<Act<S>> {
    radios(
        place,
        T::all().iter().map(|&(l, v)| {
            let n = note(v);
            (
                if n.is_empty() {
                    l.to_string()
                } else {
                    format!("{l} ({n})")
                },
                v,
            )
        }),
    )
}

/// A horizontal slider. See [`slider`].
pub struct Slide<M> {
    range: (f32, f32),
    value: f32,
    step: Option<f32>,
    on: Option<Box<dyn Fn(f32) -> M>>,
    id: u64,
    wide: bool,
    label: Option<String>,
    unit: Option<String>,
}
/// A slider over `range` showing `value`.
#[track_caller]
pub fn slider<M>(range: std::ops::RangeInclusive<f32>, value: impl Val<f32, M>) -> Slide<M> {
    let (value, on) = value.split();
    Slide {
        range: (*range.start(), *range.end()),
        value,
        step: None,
        on,
        id: auto_id(7),
        wide: true,
        label: None,
        unit: None,
    }
}
impl<M> Slide<M> {
    /// Snap to multiples of `s`.
    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
    /// Put a label in front of the slider, aligned with other labelled sliders.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Show the value after the slider, followed by `unit`: `.unit("min")` reads `25 min`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = Some(u.into());
        self
    }
}
value_widget!(
    Slide,
    "slider",
    f32,
    "Called with the new value while dragging."
);
impl<M: 'static> From<Slide<M>> for Node<M> {
    fn from(s: Slide<M>) -> Self {
        let mut t = w::Slider::new(&cx::ui(), s.range.0, s.range.1, s.value).id(s.id);
        if let Some(st) = s.step {
            t = t.step(st);
        }
        if let Some(f) = s.on {
            t = t.on_change(f);
        }
        if s.wide {
            t = t.width(Length::Fill(1));
        }
        let bar = Node(t.into_element());
        if s.label.is_none() && s.unit.is_none() {
            return bar;
        }
        let mut cells = vec![];
        if let Some(l) = s.label {
            cells.push(text(l).w(110.0).into());
        }
        cells.push(bar);
        if let Some(u) = s.unit {
            cells.push(text(format!("{} {u}", s.value)).w(64.0).into());
        }
        crate::view::row(cells).align_center().into()
    }
}

/// A progress bar over `range`.
pub fn progress<M: 'static>(range: std::ops::RangeInclusive<f32>, value: f32) -> Node<M> {
    Node(
        w::ProgressBar::new(&cx::ui(), *range.start(), *range.end(), value)
            .width(Length::Fill(1))
            .into_element(),
    )
}

/// A segmented control over fixed choices. See [`choose`] and [`tabs`].
pub struct Seg<M> {
    labels: Vec<String>,
    selected: usize,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}
/// A segmented control selecting an index.
#[track_caller]
pub fn tabs<M, L: Into<String>>(
    labels: impl IntoIterator<Item = L>,
    selected: impl Val<usize, M>,
) -> Seg<M> {
    let (selected, on) = selected.split();
    Seg {
        labels: labels.into_iter().map(Into::into).collect(),
        selected,
        on,
        id: auto_id(8),
    }
}
value_widget!(Seg, "tabs", usize, "Called with the picked index.");
impl<M: 'static> From<Seg<M>> for Node<M> {
    fn from(s: Seg<M>) -> Self {
        let mut t = w::Segmented::new(&cx::ui(), s.id, s.labels, s.selected);
        if let Some(f) = s.on {
            t = t.on_select(f);
        }
        Node(t.into_element())
    }
}

/// A segmented control over typed values, so no index bookkeeping:
/// `choose([("All", Filter::All), ("Open", Filter::Open)], &self.filter).on(Msg::Filter)`.
pub struct Choose<T, M> {
    opts: Vec<(String, T)>,
    cur: usize,
    on: Option<Box<dyn Fn(T) -> M>>,
    id: u64,
}
/// See `Choose`. `current` is matched against the options with `==`.
#[track_caller]
pub fn choose<T: PartialEq, M, L: Into<String>>(
    opts: impl IntoIterator<Item = (L, T)>,
    current: impl Val<T, M>,
) -> Choose<T, M> {
    let (current, on) = current.split();
    let opts: Vec<(String, T)> = opts.into_iter().map(|(l, v)| (l.into(), v)).collect();
    let cur = opts
        .iter()
        .position(|(_, v)| *v == current)
        .unwrap_or(usize::MAX);
    Choose {
        opts,
        cur,
        on,
        id: auto_id(9),
    }
}
impl<T, M> Choose<T, M> {
    /// Called with the picked value.
    pub fn on(mut self, f: impl Fn(T) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}
impl<T: Clone + Send + Sync + 'static, S: 'static> Choose<T, Act<S>> {
    /// Write the picked value into a [`Local`](crate::Local) view-state cell.
    pub fn local(self, cell: &impl crate::Cell<T>) -> Self {
        let cell = *cell;
        self.act(move |_, v| cell.write(v))
    }
    /// Closure style: `.act(|state, picked| ..)`.
    pub fn act(self, f: impl Fn(&mut S, T) + Send + Sync + 'static) -> Self {
        self.on(set_with(f))
    }
    /// Closure style: write the picked value into a field, `.bind(|s| &mut s.mode)`.
    pub fn bind(
        self,
        lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
    ) -> Self {
        self.on(set_via(lens))
    }
}
impl<T: Clone + 'static, M: 'static> From<Choose<T, M>> for Node<M> {
    fn from(c: Choose<T, M>) -> Self {
        let labels: Vec<String> = c.opts.iter().map(|(l, _)| l.clone()).collect();
        let vals: Vec<T> = c.opts.into_iter().map(|(_, v)| v).collect();
        let mut t = w::Segmented::new(&cx::ui(), c.id, labels, c.cur);
        if let Some(f) = c.on {
            t = t.on_select(move |i| f(vals[i].clone()));
        }
        Node(t.into_element())
    }
}

/// A drop-down list. See [`pick`].
pub struct Pick<M> {
    opts: Vec<String>,
    selected: Option<usize>,
    hint: String,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}
/// A drop-down over `options`.
#[track_caller]
pub fn pick<M, L: Into<String>>(
    options: impl IntoIterator<Item = L>,
    selected: Option<usize>,
) -> Pick<M> {
    Pick {
        opts: options.into_iter().map(Into::into).collect(),
        selected,
        hint: String::new(),
        on: None,
        id: auto_id(10),
    }
}
impl<M> Pick<M> {
    /// Text shown when nothing is selected.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
}
value_widget!(Pick, "pick", usize, "Called with the picked index.");
impl<M: 'static> From<Pick<M>> for Node<M> {
    fn from(p: Pick<M>) -> Self {
        let mut t = w::PickList::new(&cx::ui(), p.id, p.opts, p.selected).placeholder(p.hint);
        if let Some(f) = p.on {
            t = t.on_select(f);
        }
        Node(t.into_element())
    }
}

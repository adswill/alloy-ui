//! Data records: describe a struct once (`#[derive(Record)]`) and get a table,
//! an edit form with validation, add/delete and search from one call.

use crate::{
    blocks::{app, field, local, store, table, Local, Pages},
    cx,
    dsl::{button, caption, check, heading, input, pick, slider, space, Act, Choice},
    view::{col as col_of, Node, View},
};
use crate::{col, row};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::Arc;

pub use alloy_ui_macros::Record;

/// What control edits a field.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// Free text.
    Text,
    /// Text that must contain an `@`.
    Email,
    /// A number on a slider.
    Number {
        /// Lowest value.
        min: f32,
        /// Highest value.
        max: f32,
        /// Slider step.
        step: f32,
    },
    /// A checkbox.
    Bool,
    /// One of a fixed list of strings (a `String` field).
    Choice(&'static [&'static str]),
    /// One variant of a [`choices!`](crate::choices) enum; the function lists the labels.
    Enum(fn() -> Vec<&'static str>),
}

/// Static description of one field of a [`Record`].
#[derive(Clone, Copy, Debug)]
pub struct FieldInfo {
    /// Column header and form label.
    pub label: &'static str,
    /// Control and validation.
    pub kind: Kind,
    /// Text must not be empty.
    pub required: bool,
}

/// A field value, whatever its type.
#[derive(Clone, Debug)]
pub enum Value {
    /// A `String` field.
    Text(String),
    /// Any numeric field.
    Num(f32),
    /// A `bool` field.
    Bool(bool),
    /// A [`choices!`](crate::choices) enum field, as the variant's position.
    Pick(usize),
}

/// A type that can be a [`Record`] field. Implemented for `String`, `bool`, the
/// number types and every [`choices!`](crate::choices) enum.
pub trait Field: Sized {
    /// The default control for this type.
    fn kind() -> Kind;
    /// This field as a [`Value`].
    fn get(&self) -> Value;
    /// Overwrite this field from a [`Value`] of the matching shape.
    fn set(&mut self, v: Value);
}

impl Field for String {
    fn kind() -> Kind {
        Kind::Text
    }
    fn get(&self) -> Value {
        Value::Text(self.clone())
    }
    fn set(&mut self, v: Value) {
        if let Value::Text(t) = v {
            *self = t;
        }
    }
}
impl Field for bool {
    fn kind() -> Kind {
        Kind::Bool
    }
    fn get(&self) -> Value {
        Value::Bool(*self)
    }
    fn set(&mut self, v: Value) {
        if let Value::Bool(b) = v {
            *self = b;
        }
    }
}
macro_rules! num_field {
    ($($t:ty),*) => {$(impl Field for $t {
        fn kind() -> Kind {
            Kind::Number { min: 0.0, max: 100.0, step: 1.0 }
        }
        fn get(&self) -> Value {
            Value::Num(*self as f32)
        }
        fn set(&mut self, v: Value) {
            if let Value::Num(n) = v {
                *self = n as $t;
            }
        }
    })*};
}
num_field!(i32, u32, i64, u64, f32, f64, usize, u8, u16, i16);
impl<T: Choice> Field for T {
    fn kind() -> Kind {
        Kind::Enum(T::labels)
    }
    fn get(&self) -> Value {
        Value::Pick(T::all().iter().position(|(_, v)| v == self).unwrap_or(0))
    }
    fn set(&mut self, v: Value) {
        if let Value::Pick(i) = v {
            if let Some((_, v)) = T::all().get(i) {
                *self = *v;
            }
        }
    }
}

/// A struct usable as a table row and an edit form. Derive it.
pub trait Record: Clone + Default + Send + Sync + Serialize + DeserializeOwned + 'static {
    /// Every field, in declaration order.
    fn fields() -> Vec<FieldInfo>;
    /// Read field `i`.
    fn get(&self, i: usize) -> Value;
    /// Write field `i`.
    fn set(&mut self, i: usize, v: Value);
}

/// The rows of a [`records`] view plus which one is selected. Put it in your state, or in a [`store`].
#[derive(Clone, Serialize, Deserialize)]
pub struct Records<T> {
    /// The rows.
    pub rows: Vec<T>,
    sel: Option<usize>,
}

impl<T> Default for Records<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T> Records<T> {
    /// Start with `rows`.
    pub fn new(rows: Vec<T>) -> Self {
        Self { rows, sel: None }
    }
}

impl<T: Record> Records<T> {
    /// Start with one row per list of values, in field order: `Records::of([vals!["Ada", 36, true]])`.
    pub fn of<const N: usize>(rows: impl IntoIterator<Item = [Value; N]>) -> Self {
        let make = |vals: [Value; N]| {
            let mut r = T::default();
            for (i, v) in vals.into_iter().enumerate() {
                r.set(i, v);
            }
            r
        };
        Self::new(rows.into_iter().map(make).collect())
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.into())
    }
}
impl<T: Choice> From<T> for Value {
    fn from(v: T) -> Self {
        Field::get(&v)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
macro_rules! num_value {
    ($($t:ty),*) => {$(impl From<$t> for Value {
        fn from(n: $t) -> Self { Value::Num(n as f32) }
    })*};
}
num_value!(i32, u32, i64, u64, f32, f64, usize);

/// A row of mixed-type values for [`Records::of`]: `vals!["Ada", 36, true]`.
#[macro_export]
macro_rules! vals {
    ($($v:expr),* $(,)?) => { [$($crate::Value::from($v)),*] };
}

/// A master-detail editor for one record type, as a page body: `crud(|| Records::of([..]))`.
/// `seed` runs only when there is no saved state.
pub fn crud<S: 'static, T: Record>(seed: impl FnOnce() -> Records<T>) -> Node<Act<S>> {
    records(&cx::scope_name(), &store(seed))
}

/// A whole one-page CRUD app: `crud_app("Books", || Records::of([..])).run()`, saved under `title`.
pub fn crud_app<T: Record>(title: &'static str, seed: impl Fn() -> Records<T> + 'static) -> Pages {
    app(title)
        .page(title, move || records(title, &store(&seed)))
        .persist()
}

fn show(f: &FieldInfo, v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Num(n) if n.fract() == 0.0 => format!("{n:.0}"),
        Value::Num(n) => format!("{n:.2}"),
        Value::Bool(b) => if *b { "Yes" } else { "No" }.into(),
        Value::Pick(i) => match f.kind {
            Kind::Enum(labels) => labels().get(*i).copied().unwrap_or("").into(),
            _ => String::new(),
        },
    }
}

/// A read-only sortable table with one column per field of `T`, headers from the derive.
pub fn table_of<T: Record, S: 'static>(rows: &[T]) -> crate::Table<'_, T, S> {
    let mut grid = table(rows);
    for (i, f) in T::fields().into_iter().enumerate() {
        let get = move |r: &T| show(&f, &r.get(i));
        grid = match f.kind {
            Kind::Number { .. } => grid.num(f.label, get),
            _ => grid.col(f.label, get),
        };
    }
    grid
}

type Edit<S, T> = Arc<dyn Fn(&mut S, &mut dyn FnMut(&mut Records<T>)) + Send + Sync>;

/// A master-detail editor for rows kept in a [`store`] (or any [`Local`]): search box,
/// sortable table, Add / Delete, and a validated form for the selected row.
pub fn records<S: 'static, T: Record>(title: &str, data: &Local<Records<T>>) -> Node<Act<S>> {
    let d = data.clone();
    let edit: Edit<S, T> = Arc::new(move |_, f| d.with(f));
    data.with(|rows| editor(title, rows, edit))
}

/// Like [`records`] for rows that live in your own state; `lens` says where: `|s| &mut s.people`.
pub fn records_in<S: 'static, T: Record>(
    title: &str,
    data: &Records<T>,
    lens: fn(&mut S) -> &mut Records<T>,
) -> Node<Act<S>> {
    editor(title, data, Arc::new(move |s, f| f(lens(s))))
}

fn editor<S: 'static, T: Record>(title: &str, data: &Records<T>, lens: Edit<S, T>) -> Node<Act<S>> {
    let fields = T::fields();
    let q = local(String::new());
    let grid = table_of(&data.rows);
    let (l1, l2, l3) = (lens.clone(), lens.clone(), lens.clone());
    let grid = grid
        .filter(&q.get())
        .selected(data.sel)
        .on_select(move |s, i| l1(s, &mut |d| d.sel = Some(i)))
        .action("Delete", move |s, i| {
            l2(s, &mut |d| {
                d.rows.remove(i);
                d.sel = None;
            })
        });

    let detail = data
        .sel
        .and_then(|r| data.rows.get(r).map(|row| (r, row)))
        .map(|(r, rec)| {
            let mut items = vec![Node::from(heading(show(&fields[0], &rec.get(0))))];
            for (i, f) in fields.iter().enumerate() {
                let lens = lens.clone();
                let put = move |v: Value| {
                    let lens = lens.clone();
                    Act::new(move |s: &mut S| lens(s, &mut |d| d.rows[r].set(i, v.clone())))
                };
                let (body, err): (Node<Act<S>>, Option<&str>) = match (f.kind, rec.get(i)) {
                    (Kind::Text | Kind::Email, Value::Text(t)) => {
                        let err = if f.required && t.is_empty() {
                            Some("Required")
                        } else if matches!(f.kind, Kind::Email) && !t.contains('@') {
                            Some("Enter a valid email")
                        } else {
                            None
                        };
                        (input(t).on(move |v| put(Value::Text(v))).into(), err)
                    }
                    (Kind::Number { min, max, step }, Value::Num(n)) => (
                        slider(min..=max, n)
                            .step(step)
                            .on(move |v| put(Value::Num(v)))
                            .into(),
                        None,
                    ),
                    (Kind::Choice(opts), Value::Text(t)) => {
                        let at = opts.iter().position(|o| *o == t);
                        let on = move |k: usize| put(Value::Text(opts[k].to_string()));
                        (pick(opts.iter().copied(), at).on(on).into(), None)
                    }
                    (Kind::Enum(labels), Value::Pick(at)) => (
                        pick(labels(), Some(at))
                            .on(move |k| put(Value::Pick(k)))
                            .into(),
                        None,
                    ),
                    (_, Value::Bool(b)) => {
                        items.push(check(f.label, b).on(move |v| put(Value::Bool(v))).into());
                        continue;
                    }
                    _ => continue,
                };
                items.push(field(f.label, body, err).into());
            }
            col_of(items).gap(14.0).pad(20.0).card().w(300.0)
        });

    col![
        row![
            heading(title),
            space(),
            input(q.get()).hint("Search").local(&q).w(220.0),
            button("Add")
                .primary()
                .act(move |s: &mut S| l3(s, &mut |d| {
                    d.rows.push(T::default());
                    d.sel = Some(d.rows.len() - 1);
                })),
        ]
        .align_center()
        .gap(12.0)
        .fill_w(),
        row![
            Node::<Act<S>>::from(grid),
            detail.map_or_else(|| caption("").into(), Node::from)
        ]
        .gap(16.0)
        .fill(),
    ]
    .gap(16.0)
    .pad(24.0)
    .fill()
    .into()
}

//! Data records: describe a struct once (`#[derive(Record)]`) and get a table,
//! an edit form with validation, add/delete and search from one call.

use crate::{
    blocks::{field, local, table},
    dsl::{button, caption, check, heading, input, pick, slider, space, Act},
    view::{col as col_of, Node, View},
};
use crate::{col, row};
use serde::{Deserialize, Serialize};

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
    /// One of a fixed list of strings.
    Choice(&'static [&'static str]),
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
}

/// A struct usable as a table row and an edit form. Derive it.
pub trait Record: Clone + Default + Send + Sync + 'static {
    /// Every field, in declaration order.
    const FIELDS: &'static [FieldInfo];
    /// Read field `i`.
    fn get(&self, i: usize) -> Value;
    /// Write field `i`.
    fn set(&mut self, i: usize, v: Value);
}

/// The rows of a [`records`] view plus which one is selected. Put it in your state.
#[derive(Clone, Serialize, Deserialize)]
pub struct Records<T> {
    /// The rows.
    pub rows: Vec<T>,
    sel: Option<usize>,
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

/// A whole CRUD app for one record type: `crud::<Person>("People", rows)`, then `.run("Admin")`.
/// State is saved between runs under `title`.
pub fn crud<T>(title: &'static str, rows: Records<T>) -> impl crate::App
where
    T: Record + Serialize + serde::de::DeserializeOwned,
{
    crate::sketch(rows, move |d: &Records<T>| {
        records(title, d, |d: &mut Records<T>| d)
    })
    .persist(title)
}

fn show(v: &Value) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::Num(n) if n.fract() == 0.0 => format!("{n:.0}"),
        Value::Num(n) => format!("{n:.1}"),
        Value::Bool(b) => if *b { "Yes" } else { "No" }.into(),
    }
}

/// A complete master-detail editor for `data`: search box, sortable table,
/// Add / Delete, and a validated form for the selected row.
///
/// `lens` says where the [`Records`] live in your state: `|s| &mut s.people`.
pub fn records<S: 'static, T: Record>(
    title: &str,
    data: &Records<T>,
    lens: fn(&mut S) -> &mut Records<T>,
) -> Node<Act<S>> {
    let q = local(String::new());
    let mut grid = table(&data.rows);
    for (i, f) in T::FIELDS.iter().enumerate() {
        let get = move |r: &T| show(&r.get(i));
        grid = match f.kind {
            Kind::Number { .. } => grid.num(f.label, get),
            _ => grid.col(f.label, get),
        };
    }
    let grid = grid
        .filter(&q.get())
        .selected(data.sel)
        .on_select(move |s, i| lens(s).sel = Some(i))
        .action("Delete", move |s, i| {
            let d = lens(s);
            d.rows.remove(i);
            d.sel = None;
        });

    let detail = data
        .sel
        .and_then(|r| data.rows.get(r).map(|row| (r, row)))
        .map(|(r, rec)| {
            let mut items = vec![Node::from(heading(show(&rec.get(0))))];
            for (i, f) in T::FIELDS.iter().enumerate() {
                let put =
                    move |v: Value| Act::new(move |s: &mut S| lens(s).rows[r].set(i, v.clone()));
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
            button("Add").primary().act(move |s: &mut S| {
                let d = lens(s);
                d.rows.push(T::default());
                d.sel = Some(d.rows.len() - 1);
            }),
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

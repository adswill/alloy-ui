//! Typed inputs: [`number`] (a text field bound to a numeric field) and [`stepper`] (`- n +`).

use crate::{
    bind::Place,
    blocks::local,
    dsl::{button, input, text, Act, Input},
    view::{Node, View},
};
use std::{fmt::Display, str::FromStr};

/// A text field bound to a number field: typing writes the parsed value into the state, and
/// text that is not a number is refused as you type, so the field never holds garbage.
///
/// ```ignore
/// number(at!(s.price)).hint("0.00")
/// ```
#[track_caller]
pub fn number<S: 'static, T>(place: Place<'_, S, T>) -> Input<Act<S>>
where
    T: FromStr + Display + PartialEq + Copy + Default + Send + Sync + 'static,
{
    let parse = |t: &str| match t.trim() {
        "" => Some(T::default()),
        t => t.parse::<T>().ok(),
    };
    let buf = local(place.cur.to_string());
    let typed = buf.get();
    let shown = if parse(&typed) == Some(*place.cur) {
        typed
    } else {
        place.cur.to_string()
    };
    let lens = place.lens;
    input(shown).on(move |t: String| {
        let (lens, v) = (lens.clone(), parse(&t));
        Act::new(move |s| {
            if let Some(v) = v {
                buf.set(t.clone());
                *lens(s) = v;
            }
        })
    })
}

/// Whole numbers a [`stepper`] can step.
pub trait Count: Copy + PartialOrd + Display + Send + Sync + 'static {
    /// Widen to `i64`.
    fn to_i64(self) -> i64;
    /// Narrow from `i64`, saturating at the type's bounds.
    fn from_i64(v: i64) -> Self;
}
macro_rules! count {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn to_i64(self) -> i64 { self as i64 }
            fn from_i64(v: i64) -> Self { v.clamp(<$t>::MIN as i64, <$t>::MAX as i64) as $t }
        }
    )*};
}
count!(i8, i16, i32, i64, isize, u8, u16, u32, usize);

/// A `- 3 +` control for a whole-number field. See [`stepper`].
pub struct Stepper<'a, S, T> {
    place: Place<'a, S, T>,
    min: Option<T>,
    max: Option<T>,
    by: i64,
}

/// A `- 3 +` control bound to a whole-number field: `stepper(at!(s.people)).min(1)`.
pub fn stepper<S: 'static, T: Count>(place: Place<'_, S, T>) -> Stepper<'_, S, T> {
    Stepper {
        place,
        min: None,
        max: None,
        by: 1,
    }
}

impl<S, T> Stepper<'_, S, T> {
    /// The smallest value; `-` is disabled there.
    pub fn min(mut self, v: T) -> Self {
        self.min = Some(v);
        self
    }
    /// The largest value; `+` is disabled there.
    pub fn max(mut self, v: T) -> Self {
        self.max = Some(v);
        self
    }
    /// Change by `n` per press instead of 1.
    pub fn by(mut self, n: i64) -> Self {
        self.by = n;
        self
    }
}

impl<S: 'static, T: Count> From<Stepper<'_, S, T>> for Node<Act<S>> {
    fn from(st: Stepper<'_, S, T>) -> Self {
        let cur = *st.place.cur;
        let go = |d: i64| {
            let lens = st.place.lens.clone();
            let (lo, hi) = (st.min, st.max);
            Act::new(move |s: &mut S| {
                let v = &mut *lens(s);
                let mut n = T::from_i64(v.to_i64() + d);
                if let Some(lo) = lo.filter(|lo| n < *lo) {
                    n = lo;
                }
                if let Some(hi) = hi.filter(|hi| n > *hi) {
                    n = hi;
                }
                *v = n;
            })
        };
        let (can_dec, can_inc) = (
            st.min.map_or(true, |m| cur > m),
            st.max.map_or(true, |m| cur < m),
        );
        crate::row![
            button("-").enabled(can_dec).on(go(-st.by)),
            text(cur.to_string()).boxed().w(36.0).center(),
            button("+").enabled(can_inc).on(go(st.by)),
        ]
        .align_center()
        .into()
    }
}

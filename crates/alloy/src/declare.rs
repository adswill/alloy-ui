//! Declaring app state with its starting values in one place, and tiny formatting helpers
//! for the numbers every app prints.

/// Declare the state struct and its starting values together. A field with `= value` starts
/// there; any other field starts at `Default::default()`. The struct gets `Default`, so
/// `sketch(Timer::default(), view)` needs no initialiser block.
///
/// ```
/// alloy_ui::state! {
///     struct Timer {
///         minutes: f32 = 25.0,
///         running: bool,
///         done: u32,
///     }
/// }
/// let t = Timer::default();
/// assert_eq!((t.minutes, t.running, t.done), (25.0, false, 0));
/// ```
#[macro_export]
macro_rules! state {
    ($(#[$m:meta])* $vis:vis struct $name:ident {
        $($(#[$fm:meta])* $fvis:vis $f:ident : $t:ty $(= $d:expr)?),* $(,)?
    }) => {
        $(#[$m])*
        $vis struct $name { $($(#[$fm])* $fvis $f: $t),* }
        impl ::std::default::Default for $name {
            fn default() -> Self {
                Self { $($f: $crate::__state_val!($($d)?)),* }
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __state_val {
    () => {
        ::std::default::Default::default()
    };
    ($d:expr) => {
        $d
    };
}

/// Whole seconds as a clock: `clock(125)` is `02:05`, and `clock(3725)` is `1:02:05`.
pub fn clock(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// An amount with two decimals and a dollar sign: `money(12.5)` is `$12.50`.
pub fn money(amount: f64) -> String {
    format!("${amount:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(125), "02:05");
        assert_eq!(clock(3725), "1:02:05");
        assert_eq!(money(12.5), "$12.50");
    }
}

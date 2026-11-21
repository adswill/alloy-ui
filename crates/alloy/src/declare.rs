//! Declaring app state with its starting values in one place, and tiny formatting helpers
//! for the numbers every app prints.

// The `state!` macro itself lives in `alloy-ui-macros` (a proc macro, so mistakes get spanned
// errors). Its doc example is here, where it can run.
#[cfg(test)]
mod state_doc {
    crate::state! {
        /// Declared with starting values.
        struct Timer {
            minutes: f32 = 25.0,
            running: bool,
            done: u32,
            title: String = "Focus",
        }
    }

    #[test]
    fn starting_values() {
        let t = Timer::default();
        assert_eq!((t.minutes, t.running, t.done), (25.0, false, 0));
        assert_eq!(t.title, "Focus");
    }
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

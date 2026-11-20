//! Tiny form validators for [`field`](crate::field): each returns the message to show, or
//! `None` when the value is fine.
//!
//! ```ignore
//! field("Email", input(at!(s.email)), valid::email(&s.email))
//! ```

/// Not empty after trimming.
pub fn required(v: &str) -> Option<&'static str> {
    v.trim().is_empty().then_some("Required")
}

/// Required, and shaped like an email (`x@y.z`).
pub fn email(v: &str) -> Option<&'static str> {
    required(v).or_else(|| {
        let ok = v
            .split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !b.ends_with('.'));
        (!ok).then_some("Enter a valid email")
    })
}

/// At least `n` characters.
pub fn min_len(v: &str, n: usize) -> Option<String> {
    (v.chars().count() < n).then(|| format!("At least {n} characters"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks() {
        assert_eq!(required("  "), Some("Required"));
        assert_eq!(email("a@b"), Some("Enter a valid email"));
        assert_eq!(email("a@b.dev"), None);
        assert_eq!(min_len("abc", 8).as_deref(), Some("At least 8 characters"));
    }
}
